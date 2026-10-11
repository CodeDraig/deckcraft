//! Paint laid-out glyphs, decorations and placeholder prompts.

use super::{ParaState, color};
use deckcraft_color::Rgba;
use deckcraft_fonts::FontDb;
use deckcraft_model::resolve::{self, Ctx};
use deckcraft_model::text::TextBody;
use deckcraft_model::{PhType, Shape};
use deckcraft_text::{Fields, Opts};
use kurbo::{Affine, BezPath, Rect};
use std::collections::HashMap;
use std::sync::Arc;
use vello_cpu::RenderContext;

/// The prompt shown in an empty placeholder, formatted like its content would be.
pub(super) fn prompt_body(rctx: &Ctx, s: &Shape, kind: PhType) -> TextBody {
    // Layout/master prompt text when the layout supplies a custom one.
    let (lp, _) = resolve::parents(rctx, s);
    let custom = lp.and_then(|l| l.ph.as_ref().filter(|p| p.has_custom_prompt).and(l.text.as_ref())).filter(|t| !t.is_empty());
    if let Some(t) = custom {
        return t.clone();
    }
    if matches!(rctx.owner, resolve::Owner::Layout | resolve::Owner::Master) {
        return TextBody::default();
    }
    let txt = kind.prompt();
    if txt.is_empty() || kind.is_footer_kind() {
        return TextBody::default();
    }
    let mut t = TextBody::from_text(txt);
    if matches!(kind, PhType::Picture | PhType::Chart | PhType::Table | PhType::Media | PhType::Diagram | PhType::ClipArt) {
        t.body.anchor = Some(deckcraft_model::text::Anchor::Bottom);
        if let Some(p) = t.paragraphs.first_mut() {
            p.props.align = Some(deckcraft_model::text::Align::Center);
            p.props.bullet = Some(deckcraft_model::text::Bullet::None);
            p.props.margin_left = Some(0.0);
            p.props.indent = Some(0.0);
            for r in &mut p.runs {
                r.props.size = Some(14.0);
            }
        }
    }
    t
}

pub(super) fn glyph(face: &deckcraft_fonts::FontFace, gid: u32) -> Arc<BezPath> {
    let key = (face.id(), gid);
    if let Some(p) = GLYPH_CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return p;
    }
    let p = FontDb::global().outline(face, gid);
    GLYPH_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 200_000 {
            c.clear();
        }
        c.insert(key, p.clone());
    });
    p
}

/// Draw a text body into shape space `m` within the text rect.
pub fn draw_text(ctx: &mut RenderContext, rctx: &Ctx, s: &Shape, body: &TextBody, tr: Rect, m: Affine, fields: &dyn Fields, prompt: Option<Rgba>) {
    let l = deckcraft_text::layout(rctx, s, body, &Opts { rect: tr, fields, prompt_color: prompt, no_shrink: false });
    draw_layout(ctx, &l, tr, m);
}

/// Draw an already laid-out text block.
pub fn draw_layout(ctx: &mut RenderContext, l: &deckcraft_text::TextLayout, tr: Rect, m: Affine) {
    draw_layout_with(ctx, l, tr, m, &[]);
}

/// Draw a laid-out text block with per-paragraph animation states.
pub fn draw_layout_with(ctx: &mut RenderContext, l: &deckcraft_text::TextLayout, tr: Rect, m: Affine, paras: &[(usize, ParaState)]) {
    let para = |i: usize| paras.iter().find(|(p, _)| *p == i).map(|(_, s)| *s).unwrap_or_default();
    let m = if l.rotation != 0.0 {
        let c = tr.center().to_vec2();
        m * Affine::translate(c) * Affine::rotate(l.rotation.to_radians()) * Affine::translate(-c)
    } else {
        m
    };
    let deco = |ctx: &mut RenderContext, d: &deckcraft_text::Deco| {
        let ps = para(d.para);
        if !ps.visible || ps.opacity <= 0.001 {
            return;
        }
        ctx.set_transform(m * Affine::translate(ps.offset));
        ctx.set_paint(color(d.color, ps.opacity));
        ctx.fill_rect(&d.rect);
    };
    for d in l.decos.iter().filter(|d| d.behind) {
        deco(ctx, d);
    }
    for run in &l.runs {
        if (run.alpha <= 0.0 || run.color.a == 0) && run.outline.is_none() {
            continue;
        }
        let ps = para(run.para);
        if !ps.visible || ps.opacity <= 0.001 {
            continue;
        }
        let m = m * Affine::translate(ps.offset);
        let alpha = run.alpha * ps.opacity;
        let k = run.size / run.face.upem.max(1.0);
        ctx.set_paint(color(run.color, alpha));
        for (gid, x, y) in &run.glyphs {
            let path = glyph(&run.face, *gid);
            if path.elements().is_empty() {
                continue;
            }
            let skew = if run.fake_italic { Affine::new([1.0, 0.0, -0.2, 1.0, 0.0, 0.0]) } else { Affine::IDENTITY };
            let gm = m * Affine::translate((*x, *y)) * skew * Affine::scale(k);
            ctx.set_transform(gm);
            if alpha > 0.0 && run.color.a > 0 {
                ctx.fill_path(&path);
                if run.fake_bold {
                    ctx.set_stroke(kurbo::Stroke::new(run.face.upem * 0.025).with_join(kurbo::Join::Round));
                    ctx.stroke_path(&path);
                }
            }
            if let Some((oc, ow)) = run.outline {
                ctx.set_paint(color(oc, ps.opacity));
                ctx.set_stroke(kurbo::Stroke::new(ow / k.max(1e-9)).with_join(kurbo::Join::Round));
                ctx.stroke_path(&path);
                ctx.set_paint(color(run.color, alpha));
            }
        }
    }
    for d in l.decos.iter().filter(|d| !d.behind) {
        deco(ctx, d);
    }
}

thread_local! {
    static GLYPH_CACHE: std::cell::RefCell<HashMap<(u32, u32), Arc<BezPath>>> = std::cell::RefCell::new(HashMap::new());
}
