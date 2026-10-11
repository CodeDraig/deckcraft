//! Text layout for slide text bodies.
//!
//! [`layout`] resolves every run's formatting through the placeholder/style chain, shapes it with
//! font fallback, breaks lines (UAX #14 opportunities, greedy fill), places bullets and numbers,
//! applies indents, spacing and alignment, anchors the block in the shape's text box, and shrinks
//! text on overflow when the body asks for it. The result is positioned glyph runs in shape-local
//! coordinates plus per-line caret positions for editing.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod caret;
pub use caret::Pos;

mod line_break;
use line_break::{break_lines, tab_advance};

mod shaping;
use shaping::{ParaShaped, line_metrics, measure_str, shape_para};

mod numbering;
pub use numbering::autonum;

use std::sync::Arc;

use deckcraft_color::Rgba;
use deckcraft_fonts::FontFace;
use deckcraft_geom::Rect;
use deckcraft_model::Shape;
use deckcraft_model::resolve::{self, Ctx};
use deckcraft_model::text::{Align, Anchor, AutoFit, BodyProps, Bullet, Caps, Spacing, Strike, TextBody, TextDir};

/// A run of glyphs in one face, size and colour.
#[derive(Clone)]
pub struct GlyphRun {
    pub face: Arc<FontFace>,
    /// Font size in points (after autofit scaling).
    pub size: f64,
    pub color: Rgba,
    /// Glyph id and pen position of its origin (baseline), in shape-local points.
    pub glyphs: Vec<(u32, f64, f64)>,
    /// Draw a heavier stroke because the family has no bold face.
    pub fake_bold: bool,
    /// Slant because the family has no italic face.
    pub fake_italic: bool,
    pub outline: Option<(Rgba, f64)>,
    /// Paragraph and character range this run covers (for hyperlinks, selection colours).
    pub para: usize,
    pub chars: (usize, usize),
    pub link: bool,
    /// Alpha of the fill (0–1).
    pub alpha: f64,
    pub gradient: Option<deckcraft_model::style::Gradient>,
}

impl std::fmt::Debug for GlyphRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlyphRun").field("family", &self.face.family).field("size", &self.size).field("glyphs", &self.glyphs.len()).finish()
    }
}

/// An underline, strikethrough or highlight box.
#[derive(Clone, Debug)]
pub struct Deco {
    pub rect: Rect,
    pub color: Rgba,
    /// Highlights draw behind text.
    pub behind: bool,
    /// Paragraph the decoration belongs to (by-paragraph animation).
    pub para: usize,
}

#[derive(Clone, Debug, Default)]
pub struct LineInfo {
    pub para: usize,
    /// Character range [start, end) in the paragraph (end excludes the line's break/newline).
    pub start: usize,
    pub end: usize,
    pub top: f64,
    pub baseline: f64,
    pub bottom: f64,
    /// Caret x for each character boundary start..=end (len = end - start + 1).
    pub caret_x: Vec<f64>,
    /// Column index (multi-column bodies).
    pub column: usize,
}

#[derive(Clone, Debug, Default)]
pub struct TextLayout {
    pub runs: Vec<GlyphRun>,
    pub decos: Vec<Deco>,
    pub lines: Vec<LineInfo>,
    /// Inner text box (insets applied), shape-local.
    pub inner: Rect,
    /// Height of the laid-out text (before anchoring).
    pub content_height: f64,
    /// Widest line.
    pub content_width: f64,
    /// Autofit font scale used (1 = none).
    pub font_scale: f64,
    pub line_reduction: f64,
    pub overflow: bool,
    /// Text rotation about the shape centre, degrees (vertical text = 90/270).
    pub rotation: f64,
    /// Per paragraph: (first line index, line count).
    pub para_lines: Vec<(usize, usize)>,
}

/// Values for fields (`slidenum`, `datetime…`, `footer`).
pub trait Fields {
    fn field(&self, kind: &str) -> Option<String>;
}

pub struct NoFields;
impl Fields for NoFields {
    fn field(&self, kind: &str) -> Option<String> {
        if kind == "slidenum" { Some("1".into()) } else { None }
    }
}

pub struct Opts<'a> {
    /// The text box: the preset's text rectangle in shape-local space.
    pub rect: Rect,
    pub fields: &'a dyn Fields,
    /// Draw this text instead of the body (placeholder prompts), in this colour.
    pub prompt_color: Option<Rgba>,
    /// Ignore autofit shrinking (used to measure natural height).
    pub no_shrink: bool,
}

fn spacing_pts(s: Option<Spacing>, size: f64, reduce: f64) -> f64 {
    match s {
        Some(Spacing::Pct(p)) => p * size * 1.2 * (1.0 - reduce),
        Some(Spacing::Pts(v)) => v * (1.0 - reduce),
        None => 0.0,
    }
}

/// Lay out a shape's text body.
pub fn layout(ctx: &Ctx, shape: &Shape, body: &TextBody, opts: &Opts) -> TextLayout {
    let bp = resolve::body(ctx, shape);
    let autofit = bp.autofit.unwrap_or(AutoFit::None);
    let mut l = layout_scaled(ctx, shape, body, &bp, opts, 1.0, 0.0);
    if let (AutoFit::Shrink { .. }, false) = (autofit, opts.no_shrink)
        && l.overflow
    {
        // Binary search the largest scale that fits (steps like 90%, 80%… with line reduction).
        let (mut lo, mut hi) = (0.1_f64, 1.0_f64);
        let mut best = None;
        for _ in 0..12 {
            let mid = (lo + hi) / 2.0;
            let red: f64 = ((1.0 - mid) * 0.8_f64).min(0.2);
            let t = layout_scaled(ctx, shape, body, &bp, opts, mid, red);
            if t.overflow {
                hi = mid;
            } else {
                lo = mid;
                best = Some(t);
            }
        }
        l = best.unwrap_or_else(|| layout_scaled(ctx, shape, body, &bp, opts, 0.1, 0.2));
    }
    l
}

fn layout_scaled(ctx: &Ctx, shape: &Shape, body: &TextBody, bp: &BodyProps, opts: &Opts, scale: f64, reduce: f64) -> TextLayout {
    let r = opts.rect;
    let vert = bp.vert.unwrap_or(TextDir::Horizontal);
    let rotation = match vert {
        TextDir::Vertical | TextDir::EaVertical | TextDir::Stacked => 90.0,
        TextDir::Vertical270 => 270.0,
        TextDir::Horizontal => 0.0,
    } + bp.rot.unwrap_or(0.0);
    // For vertical text, lay out in a box with swapped dimensions centred on the text rect.
    let r = if (rotation - 90.0).abs() < 1e-6 || (rotation - 270.0).abs() < 1e-6 {
        let c = r.center();
        Rect::new(c.x - r.height() / 2.0, c.y - r.width() / 2.0, c.x + r.height() / 2.0, c.y + r.width() / 2.0)
    } else {
        r
    };
    let inner = Rect::new(
        r.x0 + bp.inset_l.unwrap_or(7.2),
        r.y0 + bp.inset_t.unwrap_or(3.6),
        (r.x1 - bp.inset_r.unwrap_or(7.2)).max(r.x0 + bp.inset_l.unwrap_or(7.2)),
        (r.y1 - bp.inset_b.unwrap_or(3.6)).max(r.y0 + bp.inset_t.unwrap_or(3.6)),
    );
    let wrap = bp.wrap.unwrap_or(true) && !matches!(bp.autofit, Some(AutoFit::Shape) if shape.text_box && !bp.wrap.unwrap_or(true));
    let ncols = bp.columns.unwrap_or(1).clamp(1, 16) as usize;
    let col_gap = bp.col_spacing.unwrap_or(0.0).max(0.0);
    let col_w = ((inner.width() - col_gap * (ncols as f64 - 1.0)) / ncols as f64).max(1.0);

    let mut out = TextLayout { inner, font_scale: scale, line_reduction: reduce, rotation, ..Default::default() };
    let mut y = 0.0;
    let mut numbering: Vec<u32> = vec![0; 9];
    let mut all_lines: Vec<(LineInfo, Vec<(usize, f64, f64)>, f64, Option<(String, f64, f64)>)> = vec![];
    let mut prev_after = 0.0;
    let mut shaped_paras: Vec<ParaShaped> = Vec::with_capacity(body.paragraphs.len());
    for (pi, para) in body.paragraphs.iter().enumerate() {
        let lvl = para.level.min(8) as usize;
        let pp = resolve::para(ctx, shape, para);
        let number = if matches!(pp.bullet, Some(Bullet::AutoNum { .. })) && !para.is_empty() {
            if let Some(n) = numbering.get_mut(lvl) {
                *n += 1;
            }
            for deeper in numbering.iter_mut().skip(lvl + 1) {
                *deeper = 0;
            }
            numbering.get(lvl).copied()
        } else {
            if !para.is_empty() {
                for n in numbering.iter_mut().skip(lvl) {
                    *n = 0;
                }
            }
            None
        };
        shaped_paras.push(shape_para(ctx, shape, body, pi, scale, opts, number));
    }
    for (pi, p) in shaped_paras.iter().enumerate() {
        let props = &p.props;
        let margin = props.margin_left.unwrap_or(0.0).max(0.0);
        let indent = props.indent.unwrap_or(0.0);
        let mr = props.margin_right.unwrap_or(0.0).max(0.0);
        let bullet_w = p.bullet.as_ref().map(|(s, st)| measure_str(&st.face, st.size, s).0).unwrap_or(0.0);
        let first_x = if p.bullet.is_some() {
            let bx = (margin + indent).max(0.0);
            if indent < 0.0 && bx + bullet_w <= margin { margin } else { bx + bullet_w + if indent >= 0.0 { 0.0 } else { 3.0 } }
        } else {
            (margin + indent).max(0.0)
        };
        let rest_x = margin;
        let tabs: Vec<f64> = props.tabs.as_ref().map(|t| t.iter().map(|s| s.pos).collect()).unwrap_or_default();
        let first_w = (col_w - first_x - mr).max(1.0);
        let rest_w = (col_w - rest_x - mr).max(1.0);
        let raw = break_lines(p, first_w, rest_w, wrap, &tabs, props.default_tab.unwrap_or(72.0), first_x, rest_x);
        let before = if pi == 0 { 0.0 } else { spacing_pts(props.space_before, p.lead_size, reduce) };
        y += before.max(0.0) + prev_after;
        let first_line_idx = all_lines.len();
        for (li, rl) in raw.iter().enumerate() {
            // Line metrics: max ascent/descent over the line's styles (or the lead style if empty).
            let (mut asc, mut desc, mut size) = (0.0f64, 0.0f64, 0.0f64);
            for c in p.cells.get(rl.start..rl.end.max(rl.start)).unwrap_or(&[]) {
                if let (Some(st), Some(face)) = (p.styles.get(c.style), p.faces.get(c.face)) {
                    let sz = st.size;
                    let (a, d) = line_metrics(face, sz);
                    asc = asc.max(a);
                    desc = desc.max(d);
                    size = size.max(sz);
                }
            }
            if size == 0.0 {
                let (a, d) = line_metrics(&p.lead_face, p.lead_size);
                asc = a;
                desc = d;
                size = p.lead_size;
            }
            let natural = (asc + desc).max(size * 1.0);
            let lh = match props.line_spacing {
                Some(Spacing::Pct(f)) => natural * f.max(0.0) * (1.0 - reduce),
                Some(Spacing::Pts(v)) => v * (1.0 - reduce),
                None => natural * (1.0 - reduce),
            };
            let ratio = if natural > 0.0 { lh / natural } else { 1.0 };
            let top = y;
            let baseline = top + asc * ratio;
            y += lh;
            let x0 = if li == 0 { first_x } else { rest_x };
            let avail = col_w - x0 - mr;
            // Positions per char.
            let mut xs = vec![];
            let mut x = 0.0;
            for i in rl.start..rl.end {
                xs.push(x);
                let c = p.cells.get(i);
                let adv = match c.map(|c| c.ch) {
                    Some('\t') => tab_advance(x0 + x, &tabs, props.default_tab.unwrap_or(72.0)),
                    _ => c.map(|c| c.adv).unwrap_or(0.0),
                };
                x += adv;
            }
            xs.push(x);
            let width = x;
            let align = props.align.unwrap_or(Align::Left);
            let last_line = li + 1 == raw.len();
            let slack = (avail - width).max(0.0);
            let (shift, extra_per_space, extra_per_char) = match align {
                Align::Left => (0.0, 0.0, 0.0),
                Align::Center => (slack / 2.0, 0.0, 0.0),
                Align::Right => (slack, 0.0, 0.0),
                Align::Justify => {
                    let spaces = (rl.start..rl.end).filter(|i| p.cells.get(*i).is_some_and(|c| c.ch == ' ')).count();
                    if last_line || spaces == 0 || !wrap { (0.0, 0.0, 0.0) } else { (0.0, slack / spaces as f64, 0.0) }
                }
                Align::Distributed => {
                    let n = rl.end.saturating_sub(rl.start);
                    if n > 1 { (0.0, 0.0, slack / (n - 1) as f64) } else { (slack / 2.0, 0.0, 0.0) }
                }
            };
            let mut adj = vec![];
            let mut extra = 0.0;
            for (k, i) in (rl.start..=rl.end).enumerate() {
                adj.push(xs.get(k).copied().unwrap_or(width) + extra + shift + x0);
                if i < rl.end {
                    if p.cells.get(i).is_some_and(|c| c.ch == ' ') {
                        extra += extra_per_space;
                    }
                    extra += extra_per_char;
                }
            }
            let mut glyph_cells = vec![];
            for (k, i) in (rl.start..rl.end).enumerate() {
                glyph_cells.push((i, adj.get(k).copied().unwrap_or(0.0), 0.0));
            }
            let bullet = if li == 0 {
                p.bullet.as_ref().map(|(s, _)| {
                    let bx = if matches!(align, Align::Center | Align::Right) && p.cells.is_empty() {
                        shift + x0 - bullet_w
                    } else {
                        (margin + indent).max(0.0) + if align == Align::Center || align == Align::Right { shift } else { 0.0 }
                    };
                    (s.clone(), bx, baseline)
                })
            } else {
                None
            };
            out.content_width = out.content_width.max(width + x0 + mr);
            all_lines.push((
                LineInfo { para: pi, start: rl.start, end: rl.end, top, baseline, bottom: y, caret_x: adj, column: 0 },
                glyph_cells,
                lh,
                bullet,
            ));
        }
        out.para_lines.push((first_line_idx, raw.len()));
        prev_after = spacing_pts(props.space_after, p.lead_size, reduce).max(0.0);
    }
    let total = y;
    out.content_height = total;
    // Columns: distribute lines by height.
    let col_h = if ncols > 1 { inner.height() } else { f64::INFINITY };
    let mut col = 0usize;
    let mut col_y0 = 0.0;
    for (li, _, _, _) in all_lines.iter_mut() {
        if ncols > 1 && li.bottom - col_y0 > col_h + 0.01 && col + 1 < ncols {
            col += 1;
            col_y0 = li.top;
        }
        li.column = col;
        if col > 0 {
            let dy = col_y0;
            li.top -= dy;
            li.baseline -= dy;
            li.bottom -= dy;
            let dx = col as f64 * (col_w + col_gap);
            for x in &mut li.caret_x {
                *x += dx;
            }
        }
    }
    let used_h = if ncols > 1 { all_lines.iter().map(|(l, ..)| l.bottom).fold(0.0, f64::max) } else { total };
    out.overflow = used_h > inner.height() + 0.5 || (ncols > 1 && all_lines.last().is_some_and(|(l, ..)| l.bottom > inner.height() + 0.5));
    let anchor = bp.anchor.unwrap_or(Anchor::Top);
    let dy = match anchor {
        Anchor::Top | Anchor::Justified | Anchor::Distributed => 0.0,
        Anchor::Middle => (inner.height() - used_h) / 2.0,
        Anchor::Bottom => inner.height() - used_h,
    };
    // anchorCtr: centre the block horizontally.
    let dx = if bp.anchor_ctr.unwrap_or(false) { ((inner.width() - out.content_width) / 2.0).max(0.0) } else { 0.0 };
    let ox = inner.x0 + dx;
    let oy = inner.y0 + dy;
    // Emit runs.
    for (line, cells, _, bullet) in all_lines.iter_mut() {
        line.top += oy;
        line.baseline += oy;
        line.bottom += oy;
        for x in &mut line.caret_x {
            *x += ox;
        }
        let Some(p) = shaped_paras.get(line.para) else { continue };
        if let Some((s, bx, by)) = bullet.take()
            && let Some((_, st)) = p.bullet.as_ref()
        {
            let (_, g) = measure_str(&st.face, st.size, &s);
            out.runs.push(GlyphRun {
                face: st.face.clone(),
                size: st.size,
                color: st.color,
                glyphs: g
                    .into_iter()
                    .map(|(gid, x)| (gid, ox + bx + x + if line.column > 0 { line.column as f64 * (col_w + col_gap) } else { 0.0 }, by + oy))
                    .collect(),
                fake_bold: false,
                fake_italic: false,
                outline: None,
                para: line.para,
                chars: (0, 0),
                link: false,
                alpha: st.alpha,
                gradient: None,
            });
        }
        // Group consecutive cells by (style, face).
        let mut cur: Option<GlyphRun> = None;
        let mut cur_key = (usize::MAX, usize::MAX);
        for (ci, x, _) in cells.iter() {
            let Some(cell) = p.cells.get(*ci) else { continue };
            let Some(st) = p.styles.get(cell.style) else { continue };
            let _ = x;
            let x = line.caret_x.get(ci.saturating_sub(line.start)).copied().unwrap_or(0.0);
            let base_y = line.baseline - st.baseline * st.size;
            let size = if st.baseline != 0.0 { st.size * 0.66 } else { st.size };
            let size = if st.caps == Caps::Small && cell.ch.is_lowercase() { size * 0.8 } else { size };
            let key = (cell.style, cell.face);
            if key != cur_key {
                if let Some(r) = cur.take() {
                    out.runs.push(r);
                }
                let Some(face) = p.faces.get(cell.face) else { continue };
                cur = Some(GlyphRun {
                    face: face.clone(),
                    size,
                    color: st.color,
                    glyphs: vec![],
                    fake_bold: st.fake_bold,
                    fake_italic: st.fake_italic,
                    outline: st.outline,
                    para: line.para,
                    chars: (*ci, *ci),
                    link: st.link,
                    alpha: st.alpha,
                    gradient: st.gradient.clone(),
                });
                cur_key = key;
            }
            if let Some(r) = cur.as_mut() {
                r.chars.1 = ci + 1;
                for (gid, gx, gy) in &cell.glyphs {
                    let k = if st.baseline != 0.0 { 0.66 } else { 1.0 };
                    r.glyphs.push((*gid, x + gx * k, base_y + gy));
                }
            }
            let adv = line.caret_x.get(ci - line.start + 1).copied().unwrap_or(x) - line.caret_x.get(ci - line.start).copied().unwrap_or(x);
            let cx = line.caret_x.get(ci - line.start).copied().unwrap_or(x);
            if let Some(hl) = st.highlight {
                out.decos.push(Deco { rect: Rect::new(cx, line.top, cx + adv, line.bottom), color: hl, behind: true, para: line.para });
            }
            if !cell.ch.is_whitespace() || st.underline.is_some() {
                if let Some(uc) = st.underline {
                    let t = (st.size * 0.06).max(0.5);
                    let uy = line.baseline + st.size * 0.12;
                    out.decos.push(Deco { rect: Rect::new(cx, uy, cx + adv, uy + t), color: uc, behind: false, para: line.para });
                }
                if let Some(sk) = st.strike {
                    let t = (st.size * 0.05).max(0.5);
                    let sy = line.baseline - st.size * 0.3;
                    out.decos.push(Deco { rect: Rect::new(cx, sy, cx + adv, sy + t), color: st.color, behind: false, para: line.para });
                    if sk == Strike::Double {
                        out.decos.push(Deco { rect: Rect::new(cx, sy - t * 2.0, cx + adv, sy - t), color: st.color, behind: false, para: line.para });
                    }
                }
            }
        }
        if let Some(r) = cur.take() {
            out.runs.push(r);
        }
    }
    out.lines = all_lines.into_iter().map(|(l, ..)| l).collect();
    // Merge adjacent decorations of the same colour on the same line.
    out.decos = merge_decos(std::mem::take(&mut out.decos));
    out
}

fn merge_decos(v: Vec<Deco>) -> Vec<Deco> {
    let mut out: Vec<Deco> = Vec::with_capacity(v.len());
    for d in v {
        if let Some(last) = out.last_mut()
            && last.color == d.color
            && last.behind == d.behind
            && (last.rect.y0 - d.rect.y0).abs() < 0.01
            && (last.rect.y1 - d.rect.y1).abs() < 0.01
            && (last.rect.x1 - d.rect.x0).abs() < 0.5
        {
            last.rect.x1 = d.rect.x1;
            continue;
        }
        out.push(d);
    }
    out
}

/// Height the shape needs so its text fits (Resize shape to fit text): text height + insets.
pub fn fit_height(ctx: &Ctx, shape: &Shape, body: &TextBody, rect: Rect, fields: &dyn Fields) -> f64 {
    let bp = resolve::body(ctx, shape);
    let l = layout(ctx, shape, body, &Opts { rect, fields, prompt_color: None, no_shrink: true });
    l.content_height + bp.inset_t.unwrap_or(3.6) + bp.inset_b.unwrap_or(3.6)
}

/// Width a non-wrapping text box needs.
pub fn fit_width(ctx: &Ctx, shape: &Shape, body: &TextBody, rect: Rect, fields: &dyn Fields) -> f64 {
    let bp = resolve::body(ctx, shape);
    let wide = Rect::new(rect.x0, rect.y0, rect.x0 + 100_000.0, rect.y1);
    let mut s2 = shape.clone();
    if let Some(t) = s2.text.as_mut() {
        t.body.wrap = Some(false);
    }
    let l = layout(ctx, &s2, body, &Opts { rect: wide, fields, prompt_color: None, no_shrink: true });
    let w = l.lines.iter().map(|li| li.caret_x.last().copied().unwrap_or(0.0) - wide.x0 - bp.inset_l.unwrap_or(7.2)).fold(0.0, f64::max);
    w + bp.inset_l.unwrap_or(7.2) + bp.inset_r.unwrap_or(7.2) + 1.0
}

#[cfg(test)]
mod tests;
