//! Paragraph alignment, lists, indentation and spacing commands.

use super::super::text::format_para;
use super::super::*;
use super::{current_para, on_param};
use crate::{Result, Session};
use deckcraft_model::text::{Align, Bullet, Spacing};
use serde_json::Value;

pub(super) fn align_to(s: &mut Session, a: Align) -> Result<Value> {
    format_para(s, &move |p| p.props.align = Some(a))
}

pub(super) fn align(s: &mut Session, p: &Value) -> Result<Value> {
    let a = match str_param(p, "align").unwrap_or("left") {
        "center" | "centre" | "ctr" => Align::Center,
        "right" | "r" => Align::Right,
        "justify" | "just" => Align::Justify,
        "distributed" | "dist" => Align::Distributed,
        _ => Align::Left,
    };
    align_to(s, a)
}

pub(super) fn bullets(s: &mut Session, p: &Value) -> Result<Value> {
    let cur = matches!(current_para(s).bullet, Some(Bullet::Char { .. }));
    let on = on_param(p, Some(cur));
    let ch = str_param(p, "char").unwrap_or("•").to_string();
    let col = color_param(p, "color");
    let size = f64_param(p, "size").map(|v| v / 100.0);
    let explicit_char = p.get("char").is_some();
    format_para(s, &move |para| {
        if on {
            para.props.bullet = Some(Bullet::Char { char: ch.clone() });
            para.props.bullet_font = if explicit_char { Some("Arial".into()) } else { para.props.bullet_font.clone() };
            if col.is_some() {
                para.props.bullet_color = col.clone();
            }
            if size.is_some() {
                para.props.bullet_size = size;
            }
            if para.props.indent.is_none_or(|i| i >= 0.0) && para.props.margin_left.is_none_or(|m| m == 0.0) {
                para.props.margin_left = Some(18.0 + 36.0 * para.level as f64);
                para.props.indent = Some(-18.0);
            }
        } else {
            para.props.bullet = Some(Bullet::None);
            para.props.indent = Some(0.0);
            para.props.margin_left = Some(36.0 * para.level as f64);
        }
    })
}

pub(super) fn numbering(s: &mut Session, p: &Value) -> Result<Value> {
    let cur = matches!(current_para(s).bullet, Some(Bullet::AutoNum { .. }));
    let on = on_param(p, Some(cur));
    let scheme = str_param(p, "scheme").unwrap_or("arabicPeriod").to_string();
    let start = p.get("start").and_then(Value::as_u64).map(|v| v.clamp(1, 32767) as u32).unwrap_or(1);
    format_para(s, &move |para| {
        if on {
            para.props.bullet = Some(Bullet::AutoNum { scheme: scheme.clone(), start_at: start });
            para.props.margin_left = Some(27.0 + 36.0 * para.level as f64);
            para.props.indent = Some(-27.0);
        } else {
            para.props.bullet = Some(Bullet::None);
            para.props.indent = Some(0.0);
            para.props.margin_left = Some(36.0 * para.level as f64);
        }
    })
}

pub(super) fn level(s: &mut Session, d: i32) -> Result<Value> {
    format_para(s, &move |p| {
        let l = (p.level as i32 + d).clamp(0, 8) as u8;
        // Explicit indents shift with the level.
        if l != p.level {
            if let Some(m) = p.props.margin_left.as_mut() {
                *m = (*m + 36.0 * d as f64).max(0.0);
            }
            p.level = l;
        }
    })
}

pub(super) fn line_spacing(s: &mut Session, p: &Value) -> Result<Value> {
    let sp =
        if let Some(pt) = f64_param(p, "pt") { Spacing::Pts(pt.clamp(0.0, 1584.0)) } else { Spacing::Pct(f64_or(p, "lines", 1.0).clamp(0.0, 9.99)) };
    format_para(s, &move |para| para.props.line_spacing = Some(sp))
}

pub(super) fn paragraph(s: &mut Session, p: &Value) -> Result<Value> {
    let a = str_param(p, "align").map(|a| match a {
        "center" => Align::Center,
        "right" => Align::Right,
        "justify" => Align::Justify,
        "distributed" => Align::Distributed,
        _ => Align::Left,
    });
    let ml = f64_param(p, "indentLeft");
    let ind = f64_param(p, "indentFirst");
    let sb = f64_param(p, "spaceBefore");
    let sa = f64_param(p, "spaceAfter");
    let ls = f64_param(p, "lineSpacing");
    let lsp = f64_param(p, "lineSpacingPt");
    format_para(s, &move |para| {
        if a.is_some() {
            para.props.align = a;
        }
        if ml.is_some() {
            para.props.margin_left = ml.map(|v| v.max(0.0));
        }
        if ind.is_some() {
            para.props.indent = ind;
        }
        if let Some(v) = sb {
            para.props.space_before = Some(Spacing::Pts(v.max(0.0)));
        }
        if let Some(v) = sa {
            para.props.space_after = Some(Spacing::Pts(v.max(0.0)));
        }
        if let Some(v) = ls {
            para.props.line_spacing = Some(Spacing::Pct(v.clamp(0.0, 9.99)));
        }
        if let Some(v) = lsp {
            para.props.line_spacing = Some(Spacing::Pts(v.max(0.0)));
        }
    })
}
