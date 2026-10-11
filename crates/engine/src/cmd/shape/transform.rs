//! Shape position, size and geometry commands.

use super::super::*;
use crate::{Result, Session};
use deckcraft_model::Geom;
use serde_json::Value;

pub(super) fn move_by(s: &mut Session, p: &Value) -> Result<Value> {
    let (dx, dy) = (f64_param(p, "dx"), f64_param(p, "dy"));
    let (ax, ay) = (f64_param(p, "x"), f64_param(p, "y"));
    edit_shapes(s, p, "shape.move", |sh| {
        if let Some(x) = sh.xfrm.as_mut() {
            x.x = ax.unwrap_or(x.x) + dx.unwrap_or(0.0);
            x.y = ay.unwrap_or(x.y) + dy.unwrap_or(0.0);
        }
        Ok(())
    })
}

pub(super) fn resize(s: &mut Session, p: &Value) -> Result<Value> {
    let (w, h) = (f64_param(p, "w").map(|v| v.max(0.0)), f64_param(p, "h").map(|v| v.max(0.0)));
    let lock = bool_or(p, "lockAspect", false);
    edit_shapes(s, p, "shape.resize", |sh| {
        if let Some(x) = sh.xfrm.as_mut() {
            let ar = if x.h > 0.0 { x.w / x.h } else { 1.0 };
            let (cx, cy) = (x.x + x.w / 2.0, x.y + x.h / 2.0);
            match (w, h, lock) {
                (Some(w), _, true) => {
                    x.w = w;
                    x.h = if ar > 0.0 { w / ar } else { x.h };
                }
                (None, Some(h), true) => {
                    x.h = h;
                    x.w = h * ar;
                }
                _ => {
                    x.w = w.unwrap_or(x.w);
                    x.h = h.unwrap_or(x.h);
                }
            }
            // Rotated shapes resize about their centre; others keep the top-left.
            if x.rot != 0.0 {
                x.x = cx - x.w / 2.0;
                x.y = cy - x.h / 2.0;
            }
        }
        Ok(())
    })
}

pub(super) fn set_bounds(s: &mut Session, p: &Value) -> Result<Value> {
    edit_shapes(s, p, "shape.setBounds", |sh| {
        if let Some(x) = sh.xfrm.as_mut() {
            x.x = f64_param(p, "x").unwrap_or(x.x);
            x.y = f64_param(p, "y").unwrap_or(x.y);
            x.w = f64_param(p, "w").unwrap_or(x.w).max(0.0);
            x.h = f64_param(p, "h").unwrap_or(x.h).max(0.0);
        }
        Ok(())
    })
}

pub(super) fn rotate(s: &mut Session, p: &Value) -> Result<Value> {
    let (abs, by) = (f64_param(p, "deg"), f64_param(p, "by"));
    edit_shapes(s, p, "shape.rotate", |sh| {
        if let Some(x) = sh.xfrm.as_mut() {
            let r = abs.unwrap_or(x.rot) + by.unwrap_or(0.0);
            x.rot = ((r % 360.0) + 360.0) % 360.0;
        }
        Ok(())
    })
}

pub(super) fn flip(s: &mut Session, p: &Value) -> Result<Value> {
    let v = str_param(p, "axis").unwrap_or("horizontal").starts_with('v');
    edit_shapes(s, p, "shape.flip", |sh| {
        if let Some(x) = sh.xfrm.as_mut() {
            if v {
                x.flip_v = !x.flip_v;
            } else {
                x.flip_h = !x.flip_h;
            }
        }
        Ok(())
    })
}

pub(super) fn adjust(s: &mut Session, p: &Value) -> Result<Value> {
    let i = usize_param(p, "index").unwrap_or(0).min(16);
    let v = f64_param(p, "value").ok_or_else(|| bad("shape.adjust", "missing `value`"))?;
    edit_shapes(s, p, "shape.adjust", |sh| {
        if let Geom::Preset { name, adj } = &mut sh.geom {
            let defaults = deckcraft_geom::preset::info(name).map(|x| x.defaults).unwrap_or(&[]);
            while adj.len() <= i {
                let k = adj.len();
                adj.push(defaults.get(k).copied().unwrap_or(0.0));
            }
            if let Some(slot) = adj.get_mut(i) {
                *slot = v;
            }
        }
        Ok(())
    })
}

pub(super) fn change(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "preset").ok_or_else(|| bad("shape.change", "missing `preset`"))?;
    let info = deckcraft_geom::preset::info(name).ok_or_else(|| bad("shape.change", format!("unknown preset `{name}`")))?;
    edit_shapes(s, p, "shape.change", |sh| {
        sh.geom = Geom::preset(info.name);
        Ok(())
    })
}
