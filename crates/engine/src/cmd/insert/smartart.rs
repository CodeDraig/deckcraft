//! SmartArt shape layout and insertion.

use super::super::*;
use super::add_shape;
use crate::{Result, Session};
use deckcraft_color::SchemeSlot;
use deckcraft_geom::Xfrm;
use deckcraft_model::text::{AutoFit, TextBody};
use deckcraft_model::{Geom, Shape, ShapeStyle};
use serde_json::{Value, json};

/// SmartArt-style graphics built from shapes (grouped), with our own layouts.
pub(super) fn smart_art(s: &mut Session, p: &Value) -> Result<Value> {
    let kind = str_param(p, "kind").unwrap_or("process").to_string();
    let items: Vec<String> =
        p.get("items").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default();
    let items = if items.is_empty() { vec!["Text".to_string(); 3] } else { items };
    let n = items.len().clamp(1, 12);
    let size = s.doc()?.doc.slide_size;
    let area = Xfrm::new(size.width * 0.1, size.height * 0.28, size.width * 0.8, size.height * 0.6);
    let slots = [SchemeSlot::Accent1, SchemeSlot::Accent2, SchemeSlot::Accent3, SchemeSlot::Accent4, SchemeSlot::Accent5, SchemeSlot::Accent6];
    let mut shapes: Vec<(String, Xfrm, usize)> = vec![];
    match kind.as_str() {
        "cycle" => {
            let r = area.h.min(area.w) * 0.38;
            let (cx, cy) = (area.x + area.w / 2.0, area.y + area.h / 2.0);
            let d = r * 0.62;
            for i in 0..n {
                let a = (i as f64 / n as f64 * 360.0 - 90.0).to_radians();
                shapes.push(("ellipse".into(), Xfrm::new(cx + r * a.cos() - d / 2.0, cy + r * a.sin() - d / 2.0, d, d), i));
            }
        }
        "hierarchy" => {
            let w = area.w / (n.max(2) - 1) as f64 * 0.8;
            shapes.push(("roundRect".into(), Xfrm::new(area.x + area.w / 2.0 - w / 2.0, area.y, w, area.h * 0.3), 0));
            for i in 1..n {
                let x = area.x + (i - 1) as f64 * area.w / (n - 1).max(1) as f64;
                shapes.push(("roundRect".into(), Xfrm::new(x + w * 0.1, area.y + area.h * 0.55, w, area.h * 0.3), i));
            }
        }
        "pyramid" => {
            let h = area.h / n as f64;
            for i in 0..n {
                let w = area.w * 0.25 + area.w * 0.75 * (i as f64 + 1.0) / n as f64;
                shapes.push(("trapezoid".into(), Xfrm::new(area.x + (area.w - w) / 2.0, area.y + i as f64 * h, w, h * 0.95), i));
            }
        }
        "matrix" => {
            let (w, h) = (area.w / 2.0, area.h / 2.0);
            for i in 0..n.min(4) {
                shapes.push((
                    "roundRect".into(),
                    Xfrm::new(area.x + (i % 2) as f64 * w + 4.0, area.y + (i / 2) as f64 * h + 4.0, w - 8.0, h - 8.0),
                    i,
                ));
            }
        }
        "list" => {
            let h = area.h / n as f64;
            for i in 0..n {
                shapes.push(("roundRect".into(), Xfrm::new(area.x, area.y + i as f64 * h, area.w, h * 0.85), i));
            }
        }
        _ => {
            let gap = 24.0;
            let w = (area.w - gap * (n as f64 - 1.0)) / n as f64;
            for i in 0..n {
                shapes.push((
                    if i + 1 < n { "homePlate".into() } else { "roundRect".into() },
                    Xfrm::new(area.x + i as f64 * (w + gap), area.y + area.h * 0.25, w, area.h * 0.5),
                    i,
                ));
            }
        }
    }
    let mut ids = vec![];
    for (preset, x, i) in shapes {
        let mut sh = Shape {
            xfrm: Some(x),
            geom: Geom::preset(&preset),
            style: Some(ShapeStyle::accent(slots.get(i % 6).copied().unwrap_or(SchemeSlot::Accent1))),
            ..Default::default()
        };
        let mut body = TextBody::from_text(items.get(i).map(String::as_str).unwrap_or(""));
        body.body.autofit = Some(AutoFit::Shrink { font_scale: 1.0, line_reduction: 0.0 });
        for para in &mut body.paragraphs {
            para.props.align = Some(deckcraft_model::text::Align::Center);
            for r in &mut para.runs {
                r.props.size = Some(20.0);
            }
        }
        sh.text = Some(body);
        ids.push(add_shape(s, sh, false)?.0);
    }
    let g = s.execute("arrange.group", &json!({"ids": ids}))?;
    if let Some(gid) = g.get("id").and_then(Value::as_u64) {
        let _ = s.execute("shape.rename", &json!({"id": gid, "name": format!("SmartArt ({kind})")}));
    }
    Ok(g)
}
