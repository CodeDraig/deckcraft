//! Character formatting commands.

use super::super::text::format_run;
use super::super::*;
use super::{current_run, on_param};
use crate::{Result, Session};
use deckcraft_model::edit as ed;
use deckcraft_model::text::{Caps, RunProps, Strike};
use deckcraft_model::{Fill, Line};
use serde_json::{Value, json};

pub(super) fn bold(s: &mut Session, p: &Value) -> Result<Value> {
    let v = on_param(p, current_run(s).bold);
    format_run(s, &move |r| r.bold = Some(v))
}

pub(super) fn italic(s: &mut Session, p: &Value) -> Result<Value> {
    let v = on_param(p, current_run(s).italic);
    format_run(s, &move |r| r.italic = Some(v))
}

pub(super) fn underline(s: &mut Session, p: &Value) -> Result<Value> {
    let cur = current_run(s).underline.is_some_and(|u| u != "none");
    let v = on_param(p, Some(cur));
    let style = str_param(p, "style").unwrap_or("sng").to_string();
    format_run(s, &move |r| r.underline = Some(if v { style.clone() } else { "none".into() }))
}

pub(super) fn strike(s: &mut Session, p: &Value) -> Result<Value> {
    let cur = current_run(s).strike.is_some_and(|x| x != Strike::None);
    let v = on_param(p, Some(cur));
    let d = bool_or(p, "double", false);
    format_run(s, &move |r| {
        r.strike = Some(if !v {
            Strike::None
        } else if d {
            Strike::Double
        } else {
            Strike::Single
        })
    })
}

pub(super) fn superscript(s: &mut Session, p: &Value) -> Result<Value> {
    let v = on_param(p, Some(current_run(s).baseline.unwrap_or(0.0) > 0.0));
    format_run(s, &move |r| r.baseline = Some(if v { 0.3 } else { 0.0 }))
}

pub(super) fn subscript(s: &mut Session, p: &Value) -> Result<Value> {
    let v = on_param(p, Some(current_run(s).baseline.unwrap_or(0.0) < 0.0));
    format_run(s, &move |r| r.baseline = Some(if v { -0.25 } else { 0.0 }))
}

pub(super) fn font(s: &mut Session, p: &Value) -> Result<Value> {
    let f = str_param(p, "family").ok_or_else(|| bad("format.font", "missing `family`"))?.to_string();
    format_run(s, &move |r| r.font = Some(f.clone()))
}

pub(super) fn size(s: &mut Session, p: &Value) -> Result<Value> {
    let v = f64_param(p, "size").ok_or_else(|| bad("format.size", "missing `size`"))?.clamp(1.0, 4000.0);
    format_run(s, &move |r| r.size = Some(v))
}

/// PowerPoint's font size list (grow/shrink steps through it).
pub const SIZES: [f64; 20] = [8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 36.0, 40.0, 44.0, 48.0, 54.0, 60.0, 66.0];

pub(super) fn step(cur: f64, up: bool) -> f64 {
    if up {
        SIZES.iter().copied().find(|v| *v > cur + 0.01).unwrap_or((cur + 8.0).min(4000.0))
    } else {
        SIZES.iter().rev().copied().find(|v| *v < cur - 0.01).unwrap_or((cur - 1.0).max(1.0))
    }
}

pub(super) fn grow_shrink(s: &mut Session, up: bool) -> Result<Value> {
    // Each run steps from its own size, like PowerPoint.
    let st = s.doc()?;
    let ctx_sizes = current_run(s).size.unwrap_or(18.0);
    let _ = st;
    let base = ctx_sizes;
    format_run(s, &move |r| {
        let cur = r.size.unwrap_or(base);
        r.size = Some(step(cur, up));
    })
}

pub(super) fn grow(s: &mut Session, _p: &Value) -> Result<Value> {
    grow_shrink(s, true)
}

pub(super) fn shrink(s: &mut Session, _p: &Value) -> Result<Value> {
    grow_shrink(s, false)
}

pub(super) fn color(s: &mut Session, p: &Value) -> Result<Value> {
    let c = color_param(p, "color").ok_or_else(|| bad("format.color", "missing or invalid `color`"))?;
    format_run(s, &move |r| r.fill = Some(Fill::solid(c.clone())))
}

pub(super) fn highlight(s: &mut Session, p: &Value) -> Result<Value> {
    let c = color_param(p, "color");
    format_run(s, &move |r| r.highlight = c.clone())
}

pub(super) fn caps(s: &mut Session, p: &Value) -> Result<Value> {
    let c = match str_param(p, "caps").unwrap_or("none") {
        "small" => Caps::Small,
        "all" => Caps::All,
        _ => Caps::None,
    };
    format_run(s, &move |r| r.caps = Some(c))
}

pub(super) fn spacing(s: &mut Session, p: &Value) -> Result<Value> {
    let v = f64_param(p, "pt").unwrap_or(0.0).clamp(-100.0, 400.0);
    format_run(s, &move |r| r.spacing = Some(v))
}

pub(super) fn clear(s: &mut Session, _p: &Value) -> Result<Value> {
    format_run(s, &|r| *r = RunProps { lang: r.lang.clone(), link: r.link.clone(), ..Default::default() })
}

pub(super) fn text_shadow(s: &mut Session, p: &Value) -> Result<Value> {
    let v = on_param(p, Some(current_run(s).shadow.is_some()));
    let sh = deckcraft_model::style::Shadow {
        color: deckcraft_model::ColorRef::rgb(deckcraft_color::Rgba::BLACK).with(deckcraft_color::ColorTransform::Alpha(43000)),
        blur: 3.0,
        dist: 2.0,
        dir: 45.0,
        inner: false,
        sx: 1.0,
        sy: 1.0,
        kx: 0.0,
        ky: 0.0,
        align: String::new(),
        rotate_with_shape: false,
    };
    format_run(s, &move |r| r.shadow = if v { Some(sh.clone()) } else { None })
}

pub(super) fn text_outline(s: &mut Session, p: &Value) -> Result<Value> {
    let c = color_param(p, "color");
    let w = f64_or(p, "width", 0.75);
    format_run(s, &move |r| r.outline = c.clone().map(|c| Line::solid(c, w)))
}

pub(super) fn case(s: &mut Session, p: &Value) -> Result<Value> {
    let mode = str_param(p, "mode").unwrap_or("sentence").to_string();
    let st = s.doc()?;
    if let Some(t) = st.selection.text.clone() {
        let (a, b) = t.ordered();
        if a == b {
            return ok();
        }
        let m = mode.clone();
        super::super::text::edit_text(s, move |body, _| {
            for pi in a.0..=b.0 {
                let Some(para) = body.paragraphs.get_mut(pi) else { continue };
                let n = para.char_len();
                let from = if pi == a.0 { a.1 } else { 0 };
                let to = if pi == b.0 { b.1 } else { n };
                let mut pos = 0;
                for r in &mut para.runs {
                    let len = r.char_len();
                    if r.kind == deckcraft_model::text::RunKind::Text {
                        let rs = from.max(pos).saturating_sub(pos);
                        let re = to.min(pos + len).saturating_sub(pos);
                        if re > rs {
                            let chars: Vec<char> = r.text.chars().collect();
                            let mid: String = chars.get(rs..re).map(|c| c.iter().collect()).unwrap_or_default();
                            let pre: String = chars.get(..rs).map(|c| c.iter().collect()).unwrap_or_default();
                            let post: String = chars.get(re..).map(|c| c.iter().collect()).unwrap_or_default();
                            r.text = format!("{pre}{}{post}", ed::change_case(&mid, &m));
                        }
                    }
                    pos += len;
                }
            }
        })?;
        return ok();
    }
    edit_shapes(s, &json!({}), "format.case", |sh| {
        if let Some(t) = sh.text.as_mut() {
            for para in &mut t.paragraphs {
                for r in &mut para.runs {
                    if r.kind == deckcraft_model::text::RunKind::Text {
                        r.text = ed::change_case(&r.text, &mode);
                    }
                }
            }
        }
        Ok(())
    })
}
