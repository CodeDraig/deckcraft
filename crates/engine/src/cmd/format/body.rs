//! Text frame formatting commands.

use super::super::text::format_body;
use super::super::*;
use crate::{Result, Session};
use deckcraft_model::text::{Anchor, AutoFit, TextDir};
use serde_json::Value;

pub(super) fn anchor(s: &mut Session, p: &Value) -> Result<Value> {
    let a = match str_param(p, "anchor").unwrap_or("top") {
        "middle" | "center" | "ctr" => Anchor::Middle,
        "bottom" | "b" => Anchor::Bottom,
        _ => Anchor::Top,
    };
    format_body(s, &move |b| b.anchor = Some(a))
}

pub(super) fn direction(s: &mut Session, p: &Value) -> Result<Value> {
    let d = match str_param(p, "dir").unwrap_or("horizontal") {
        "rotate90" | "vert" => TextDir::Vertical,
        "rotate270" | "vert270" => TextDir::Vertical270,
        "stacked" => TextDir::Stacked,
        _ => TextDir::Horizontal,
    };
    format_body(s, &move |b| b.vert = Some(d))
}

pub(super) fn columns(s: &mut Session, p: &Value) -> Result<Value> {
    let n = p.get("count").and_then(Value::as_u64).unwrap_or(1).clamp(1, 16) as u32;
    let sp = f64_or(p, "spacing", 0.0).max(0.0);
    format_body(s, &move |b| {
        b.columns = Some(n);
        b.col_spacing = Some(sp);
    })
}

pub(super) fn autofit(s: &mut Session, p: &Value) -> Result<Value> {
    let m = match str_param(p, "mode").unwrap_or("none") {
        "shrink" => AutoFit::Shrink { font_scale: 1.0, line_reduction: 0.0 },
        "resize" | "shape" => AutoFit::Shape,
        _ => AutoFit::None,
    };
    format_body(s, &move |b| b.autofit = Some(m))
}

pub(super) fn wrap(s: &mut Session, p: &Value) -> Result<Value> {
    let on = bool_or(p, "on", true);
    format_body(s, &move |b| b.wrap = Some(on))
}

pub(super) fn margins(s: &mut Session, p: &Value) -> Result<Value> {
    let (l, t, r, b) = (f64_param(p, "left"), f64_param(p, "top"), f64_param(p, "right"), f64_param(p, "bottom"));
    format_body(s, &move |bp| {
        if l.is_some() {
            bp.inset_l = l.map(|v| v.max(0.0));
        }
        if t.is_some() {
            bp.inset_t = t.map(|v| v.max(0.0));
        }
        if r.is_some() {
            bp.inset_r = r.map(|v| v.max(0.0));
        }
        if b.is_some() {
            bp.inset_b = b.map(|v| v.max(0.0));
        }
    })
}
