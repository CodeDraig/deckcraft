//! Resolve run styling and shape paragraph characters with font fallback.

use super::Opts;
use super::numbering::autonum;
use deckcraft_color::Rgba;
use deckcraft_fonts::{FontDb, FontFace};
use deckcraft_model::resolve::{self, Ctx};
use deckcraft_model::text::{Bullet, Caps, ParaProps, RunKind, RunProps, Strike, TextBody};
use deckcraft_model::{Fill, Shape};
use std::sync::Arc;

pub(super) struct Style {
    pub(super) face: Arc<FontFace>,
    pub(super) size: f64,
    pub(super) color: Rgba,
    pub(super) alpha: f64,
    pub(super) fake_bold: bool,
    pub(super) fake_italic: bool,
    pub(super) baseline: f64,
    pub(super) spacing: f64,
    pub(super) caps: Caps,
    pub(super) underline: Option<Rgba>,
    pub(super) strike: Option<Strike>,
    pub(super) highlight: Option<Rgba>,
    pub(super) outline: Option<(Rgba, f64)>,
    pub(super) link: bool,
    pub(super) gradient: Option<deckcraft_model::style::Gradient>,
}

/// One shaped character cell of a paragraph.
#[derive(Clone)]
pub(super) struct Cell {
    pub(super) ch: char,
    pub(super) style: usize,
    /// Glyphs anchored at this char (gid, x offset from char start, y offset).
    pub(super) glyphs: Vec<(u32, f64, f64)>,
    pub(super) adv: f64,
    pub(super) face: usize,
}

pub(super) struct ParaShaped {
    pub(super) cells: Vec<Cell>,
    pub(super) styles: Vec<Style>,
    pub(super) faces: Vec<Arc<FontFace>>,
    pub(super) props: ParaProps,
    /// Size of the paragraph's first run (spacing percentages, empty lines).
    pub(super) lead_size: f64,
    pub(super) lead_face: Arc<FontFace>,
    pub(super) bullet: Option<(String, Style)>,
    #[allow(dead_code)]
    pub(super) level: u8,
}

pub(super) fn style_name(bold: bool, italic: bool) -> &'static str {
    match (bold, italic) {
        (true, true) => "Bold Italic",
        (true, false) => "Bold",
        (false, true) => "Italic",
        (false, false) => "Regular",
    }
}

pub(super) fn make_style(ctx: &Ctx, r: &RunProps, scale: f64, prompt: Option<Rgba>) -> Style {
    let family = resolve::font_family(ctx, r);
    let bold = r.bold.unwrap_or(false);
    let italic = r.italic.unwrap_or(false);
    let face = FontDb::global().face(&family, style_name(bold, italic));
    let fake_bold = bold && face.weight < 550.0;
    let fake_italic = italic && !face.italic;
    let size = (r.size.unwrap_or(18.0) * scale).clamp(0.5, 4000.0);
    let (mut color, alpha, gradient) = match &r.fill {
        Some(Fill::Solid { color }) => (ctx.color(color, None), color.alpha(), None),
        Some(Fill::Gradient(g)) => (g.stops.first().map(|s| ctx.color(&s.color, None)).unwrap_or(Rgba::BLACK), 1.0, Some(g.clone())),
        Some(Fill::None) => (Rgba::TRANSPARENT, 0.0, None),
        _ => (resolve::text_color(ctx, r), 1.0, None),
    };
    if let Some(p) = prompt {
        color = p;
    }
    let underline = r.underline.as_deref().filter(|u| *u != "none").map(|_| r.underline_color.as_ref().map(|c| ctx.color(c, None)).unwrap_or(color));
    let outline = r.outline.as_ref().and_then(|l| match &l.fill {
        Some(Fill::Solid { color: c }) => Some((ctx.color(c, None), l.width.unwrap_or(0.75))),
        _ => None,
    });
    Style {
        face,
        size,
        color,
        alpha,
        fake_bold,
        fake_italic,
        baseline: r.baseline.unwrap_or(0.0),
        spacing: r.spacing.unwrap_or(0.0) * scale,
        caps: r.caps.unwrap_or(Caps::None),
        underline,
        strike: r.strike.filter(|s| *s != Strike::None),
        highlight: r.highlight.as_ref().map(|c| ctx.color(c, None)),
        outline,
        link: r.link.is_some(),
        gradient,
    }
}

pub(super) fn shape_para(ctx: &Ctx, shape: &Shape, body: &TextBody, pi: usize, scale: f64, opts: &Opts, number: Option<u32>) -> ParaShaped {
    let db = FontDb::global();
    let Some(para) = body.paragraphs.get(pi) else {
        let face = db.face("Inter", "Regular");
        return ParaShaped {
            cells: vec![],
            styles: vec![],
            faces: vec![],
            props: ParaProps::default(),
            lead_size: 18.0,
            lead_face: face,
            bullet: None,
            level: 0,
        };
    };
    let props = resolve::para(ctx, shape, para);
    let mut styles: Vec<Style> = vec![];
    let mut faces: Vec<Arc<FontFace>> = vec![];
    let mut cells: Vec<Cell> = vec![];
    let face_index = |faces: &mut Vec<Arc<FontFace>>, f: &Arc<FontFace>| -> usize {
        if let Some(i) = faces.iter().position(|x| Arc::ptr_eq(x, f)) {
            return i;
        }
        faces.push(f.clone());
        faces.len() - 1
    };
    for run in &para.runs {
        let rp = resolve::run(ctx, shape, para, &run.props);
        let st = make_style(ctx, &rp, scale, opts.prompt_color);
        let text: String = match &run.kind {
            RunKind::Text => run.text.clone(),
            RunKind::Break => "\u{b}".into(),
            RunKind::Field { field } => opts.fields.field(field).unwrap_or_else(|| run.text.clone()),
            RunKind::Math { .. } => run.text.clone(),
        };
        let si = styles.len();
        // Caps mapping and small-caps sizing.
        let small = st.caps == Caps::Small;
        let upper = st.caps != Caps::None;
        styles.push(st);
        let Some(stl) = styles.get(si) else { continue };
        // Split into face segments by coverage.
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars.get(i).copied().unwrap_or(' ');
            let base = &stl.face;
            let face = if c.is_control() || c.is_whitespace() || base.covers(c) {
                base.clone()
            } else {
                db.fallback_for(c, base.id()).unwrap_or_else(|| base.clone())
            };
            let mut j = i + 1;
            while j < chars.len() {
                let d = chars.get(j).copied().unwrap_or(' ');
                let same = if d.is_whitespace() || d.is_control() {
                    true
                } else if Arc::ptr_eq(&face, base) {
                    base.covers(d)
                } else {
                    !base.covers(d) && face.covers(d)
                };
                if !same {
                    break;
                }
                j += 1;
            }
            let seg: String = chars.get(i..j).map(|s| s.iter().collect()).unwrap_or_default();
            let fi = face_index(&mut faces, &face);
            let upem = face.upem.max(1.0);
            let sz = |ch: char| if small && ch.is_lowercase() { stl.size * 0.8 } else { stl.size };
            let shaped = deckcraft_fonts::shape(&face, &seg, &[], |ch| if upper { ch.to_uppercase().next().unwrap_or(ch) } else { ch });
            // byte offset → char index within seg
            let mut byte_to_char = vec![0usize; seg.len() + 1];
            for (ci, (b, _)) in seg.char_indices().enumerate() {
                if let Some(x) = byte_to_char.get_mut(b) {
                    *x = ci;
                }
            }
            let start_cell = cells.len();
            for ch in seg.chars() {
                cells.push(Cell { ch, style: si, glyphs: vec![], adv: 0.0, face: fi });
            }
            for g in shaped {
                let ci = byte_to_char.get(g.cluster).copied().unwrap_or(0);
                if let Some(cell) = cells.get_mut(start_cell + ci) {
                    let s = sz(cell.ch) / upem;
                    let ch = cell.ch;
                    if ch == '\t' || ch == '\u{b}' || ch == '\n' {
                        continue;
                    }
                    cell.glyphs.push((g.gid, cell.adv + g.x_offset as f64 * s, -(g.y_offset as f64) * s));
                    cell.adv += g.x_advance as f64 * s;
                }
            }
            for cell in cells.iter_mut().skip(start_cell) {
                if !cell.glyphs.is_empty() || cell.ch == ' ' {
                    cell.adv += stl.spacing;
                }
            }
            i = j;
        }
    }
    let lead = para.runs.first().map(|r| resolve::run(ctx, shape, para, &r.props)).unwrap_or_else(|| resolve::run(ctx, shape, para, &para.end_props));
    let lead_style = make_style(ctx, &lead, scale, opts.prompt_color);
    let lead_size = lead_style.size;
    let lead_face = lead_style.face.clone();
    // Bullet.
    let bullet = if para.is_empty() && opts.prompt_color.is_none() {
        None
    } else {
        match &props.bullet {
            Some(Bullet::Char { char }) => {
                let mut rp = lead.clone();
                if let Some(f) = &props.bullet_font {
                    rp.font = Some(f.clone());
                }
                rp.bold = Some(false);
                rp.italic = Some(false);
                if let Some(c) = &props.bullet_color {
                    rp.fill = Some(Fill::solid(c.clone()));
                }
                rp.size = Some(lead.size.unwrap_or(18.0) * props.bullet_size.unwrap_or(1.0));
                rp.underline = None;
                rp.strike = None;
                rp.highlight = None;
                let mut st = make_style(ctx, &rp, scale, opts.prompt_color);
                st.baseline = 0.0;
                // Symbol fonts (Wingdings) aren't available: map common private-use bullets.
                let ch = map_symbol_bullet(char, props.bullet_font.as_deref());
                if !st.face.covers(ch.chars().next().unwrap_or('•')) {
                    st.face = db.face("Inter", "Regular");
                }
                Some((ch, st))
            }
            Some(Bullet::AutoNum { scheme, start_at }) => {
                let mut rp = lead.clone();
                if let Some(c) = &props.bullet_color {
                    rp.fill = Some(Fill::solid(c.clone()));
                }
                rp.size = Some(lead.size.unwrap_or(18.0) * props.bullet_size.unwrap_or(1.0));
                rp.underline = None;
                let st = make_style(ctx, &rp, scale, opts.prompt_color);
                Some((autonum(scheme, start_at.saturating_sub(1).saturating_add(number.unwrap_or(1))), st))
            }
            _ => None,
        }
    };
    ParaShaped { cells, styles, faces, props, lead_size, lead_face, bullet, level: para.level }
}

pub(super) fn map_symbol_bullet(c: &str, font: Option<&str>) -> String {
    let f = font.unwrap_or("").to_ascii_lowercase();
    let first = c.chars().next().unwrap_or('•');
    if f.contains("wingdings") || f.contains("symbol") || ('\u{f000}'..='\u{f0ff}').contains(&first) {
        let code = (first as u32) & 0xff;
        let m = match code {
            0xa7 => '▪',
            0xd8 => '➢',
            0xfc => '✓',
            0x76 => '❖',
            0x71 => '❑',
            0x6e => '■',
            0x6c => '●',
            0xa8 => '◆',
            0xb7 => '•',
            0x2d => '–',
            _ => '•',
        };
        return m.to_string();
    }
    c.to_string()
}

pub(super) fn measure_str(face: &FontFace, size: f64, s: &str) -> (f64, Vec<(u32, f64)>) {
    let g = deckcraft_fonts::shape(face, s, &[], |c| c);
    let k = size / face.upem.max(1.0);
    let mut x = 0.0;
    let mut out = vec![];
    for gl in g {
        out.push((gl.gid, x + gl.x_offset as f64 * k));
        x += gl.x_advance as f64 * k;
    }
    (x, out)
}

pub(super) fn line_metrics(face: &FontFace, size: f64) -> (f64, f64) {
    let k = size / face.upem.max(1.0);
    (face.ascent * k, face.descent * k)
}
