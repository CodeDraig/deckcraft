//! Shape paint, outline and effects commands.

use super::super::*;
use super::quick_style_for;
use crate::{Result, Session};
use deckcraft_color::{ColorTransform, Rgba, SchemeSlot};
use deckcraft_model::style::{Dash, Glow, Gradient, GradientShape, GradientStop, LineEnd, PatternFill, PictureFill, Reflection, Shadow};
use deckcraft_model::{ColorRef, Effects, Fill, Line};
use serde_json::Value;

/// `{stops: [[pos 0–1, color], …], kind?: linear|radial|rectangular|path, angle?: deg (linear),
/// focus?: [l, t, r, b] fractions (radial/rectangular/path: where the first stop sits),
/// rotateWithShape?}`.
pub(crate) fn gradient_param(v: &Value) -> Option<Gradient> {
    let stops: Vec<GradientStop> = v
        .get("stops")?
        .as_array()?
        .iter()
        .filter_map(|s| {
            let a = s.as_array()?;
            Some(GradientStop { pos: a.first()?.as_f64()?.clamp(0.0, 1.0), color: color_value(a.get(1)?)? })
        })
        .collect();
    if stops.len() < 2 {
        return None;
    }
    let focus = v
        .get("focus")
        .and_then(Value::as_array)
        .and_then(|a| {
            let f = |i: usize| a.get(i).and_then(Value::as_f64).filter(|x| x.is_finite()).map(|x| x.clamp(0.0, 1.0));
            Some([f(0)?, f(1)?, f(2)?, f(3)?])
        })
        .unwrap_or([0.5, 0.5, 0.5, 0.5]);
    let mut stops = stops;
    stops.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    let path = |p: &str| GradientShape::Path { path: p.into(), focus };
    let shape = match v.get("kind").and_then(Value::as_str) {
        Some("radial" | "circle") => path("circle"),
        Some("rectangular" | "rect") => path("rect"),
        Some("path" | "shape") => path("shape"),
        _ => GradientShape::Linear {
            angle: v.get("angle").and_then(Value::as_f64).filter(|a| a.is_finite()).unwrap_or(90.0).rem_euclid(360.0),
            scaled: false,
        },
    };
    Some(Gradient { stops, shape, rotate_with_shape: v.get("rotateWithShape").and_then(Value::as_bool).unwrap_or(true) })
}

pub(super) fn fill(s: &mut Session, p: &Value) -> Result<Value> {
    let new_fill: Option<Option<Fill>> = if bool_or(p, "reset", false) {
        Some(None)
    } else if bool_or(p, "none", false) {
        Some(Some(Fill::None))
    } else if let Some(c) = color_param(p, "color") {
        Some(Some(Fill::solid(c)))
    } else if let Some(g) = p.get("gradient") {
        Some(Some(Fill::Gradient(
            gradient_param(g).ok_or_else(|| bad("shape.fill", "gradient needs at least two `stops`: [[0, \"#fff\"], [1, \"#000\"]]"))?,
        )))
    } else if let Some(pt) = p.get("pattern") {
        Some(Some(Fill::Pattern(PatternFill {
            preset: pt.get("preset").and_then(Value::as_str).unwrap_or("pct50").to_string(),
            fg: pt.get("fg").and_then(color_value).unwrap_or(ColorRef::scheme(SchemeSlot::Accent1)),
            bg: pt.get("bg").and_then(color_value).unwrap_or(ColorRef::scheme(SchemeSlot::Bg1)),
        })))
    } else if p.get("picture").is_some() {
        let (name, bytes) = super::super::insert::media_bytes(&with_param(p, "data", p.get("picture").cloned().unwrap_or_default()), "shape.fill")?;
        let ct = super::super::insert::content_type(&name, &bytes);
        let media = s.edit(|doc, _| Ok(doc.add_media(&name, ct, bytes)))?;
        Some(Some(Fill::Picture(PictureFill { media, ..Default::default() })))
    } else {
        None
    };
    let transparency = f64_param(p, "transparency");
    if new_fill.is_none() && transparency.is_none() {
        return Err(bad("shape.fill", "give `color`, `none`, `gradient`, `pattern`, `picture`, `transparency` or `reset`"));
    }
    // Transparency on an inherited (style) fill needs the effective colour.
    let st = s.doc()?;
    let ctx_fill: Vec<(deckcraft_model::ShapeId, Option<Fill>)> = {
        let ctx = super::super::text::ctx_for(&st.doc, &st.selection);
        targets(s, p)?
            .into_iter()
            .filter_map(|id| {
                st.shape(id).map(|sh| {
                    (
                        id,
                        ctx.as_ref().and_then(|c| {
                            let (f, ph) = deckcraft_model::resolve::fill(c, sh);
                            f.map(|f| match f {
                                Fill::Solid { color } => Fill::solid(ColorRef::rgb(c.color(&color, ph))),
                                other => other,
                            })
                        }),
                    )
                })
            })
            .collect()
    };
    edit_shapes(s, p, "shape.fill", |sh| {
        if let Some(f) = &new_fill {
            sh.fill.clone_from(f);
        }
        if let Some(t) = transparency {
            let base = sh.fill.clone().or_else(|| ctx_fill.iter().find(|(i, _)| *i == sh.id).and_then(|(_, f)| f.clone()));
            if let Some(Fill::Solid { mut color }) = base {
                color.mods.retain(|m| !matches!(m, ColorTransform::Alpha(_)));
                if t > 0.0 {
                    color.mods.push(ColorTransform::Alpha(((1.0 - t.clamp(0.0, 1.0)) * 100_000.0).round() as i32));
                }
                sh.fill = Some(Fill::Solid { color });
            }
        }
        Ok(())
    })
}

pub(super) fn line_end(v: &str) -> Option<LineEnd> {
    if v == "none" { None } else { Some(LineEnd { kind: v.to_string(), w: "med".into(), len: "med".into() }) }
}

pub(super) fn line(s: &mut Session, p: &Value) -> Result<Value> {
    let reset = bool_or(p, "reset", false);
    let none = bool_or(p, "none", false);
    let color = color_param(p, "color");
    let width = f64_param(p, "width").map(|w| w.clamp(0.0, 1584.0));
    let dash = str_param(p, "dash").map(|d| Dash::Preset { name: d.to_string() });
    let head = str_param(p, "head").map(line_end);
    let tail = str_param(p, "tail").map(line_end);
    let cap = str_param(p, "cap").map(|c| match c {
        "round" => deckcraft_model::style::LineCap::Round,
        "square" => deckcraft_model::style::LineCap::Square,
        _ => deckcraft_model::style::LineCap::Flat,
    });
    let join = str_param(p, "join").map(|c| match c {
        "bevel" => deckcraft_model::style::LineJoin::Bevel,
        "miter" => deckcraft_model::style::LineJoin::Miter,
        _ => deckcraft_model::style::LineJoin::Round,
    });
    edit_shapes(s, p, "shape.line", |sh| {
        if reset {
            sh.line = None;
            return Ok(());
        }
        let l = sh.line.get_or_insert_with(Line::default);
        if none {
            l.fill = Some(Fill::None);
        }
        if let Some(c) = &color {
            l.fill = Some(Fill::solid(c.clone()));
        }
        if width.is_some() {
            l.width = width;
            if l.fill.is_none() || matches!(l.fill, Some(Fill::None)) && !none {
                l.fill = Some(Fill::solid(ColorRef::scheme(SchemeSlot::Tx1)));
            }
        }
        if dash.is_some() {
            l.dash.clone_from(&dash);
        }
        if let Some(h) = &head {
            l.head.clone_from(h);
        }
        if let Some(t) = &tail {
            l.tail.clone_from(t);
        }
        if cap.is_some() {
            l.cap = cap;
        }
        if join.is_some() {
            l.join = join;
        }
        Ok(())
    })
}

pub(super) fn shadow_preset(v: &Value) -> Option<Option<Shadow>> {
    let black = |a: i32| ColorRef::rgb(Rgba::BLACK).with(ColorTransform::Alpha(a));
    let base = |inner: bool| Shadow {
        color: black(40000),
        blur: 4.0,
        dist: 3.0,
        dir: 45.0,
        inner,
        sx: 1.0,
        sy: 1.0,
        kx: 0.0,
        ky: 0.0,
        align: "tl".into(),
        rotate_with_shape: false,
    };
    match v {
        Value::String(s) => Some(match s.as_str() {
            "none" => None,
            "inner" => Some(base(true)),
            "perspective" => Some(Shadow { sy: 0.23, kx: -1200000.0 / 60000.0, dir: 90.0, dist: 0.0, blur: 6.0, align: "bl".into(), ..base(false) }),
            "bottom" => Some(Shadow { dir: 90.0, ..base(false) }),
            "center" => Some(Shadow { dist: 0.0, blur: 8.0, ..base(false) }),
            _ => Some(base(false)),
        }),
        Value::Object(o) => Some(Some(Shadow {
            color: o.get("color").and_then(color_value).unwrap_or(black(40000)),
            blur: o.get("blur").and_then(Value::as_f64).unwrap_or(4.0).clamp(0.0, 200.0),
            dist: o.get("dist").and_then(Value::as_f64).unwrap_or(3.0).clamp(0.0, 200.0),
            dir: o.get("dir").and_then(Value::as_f64).unwrap_or(45.0),
            inner: o.get("inner").and_then(Value::as_bool).unwrap_or(false),
            ..base(false)
        })),
        _ => None,
    }
}

pub(super) fn effects(s: &mut Session, p: &Value) -> Result<Value> {
    let reset = bool_or(p, "reset", false);
    let shadow = p.get("shadow").and_then(shadow_preset);
    let glow: Option<Option<Glow>> = p.get("glow").map(|g| match g {
        Value::Object(o) => Some(Glow {
            color: o.get("color").and_then(color_value).unwrap_or(ColorRef::scheme(SchemeSlot::Accent1).with(ColorTransform::Alpha(40000))),
            radius: o.get("radius").and_then(Value::as_f64).unwrap_or(8.0).clamp(0.0, 150.0),
        }),
        Value::Number(n) => Some(Glow {
            color: ColorRef::scheme(SchemeSlot::Accent1).with(ColorTransform::Alpha(40000)),
            radius: n.as_f64().unwrap_or(8.0).clamp(0.0, 150.0),
        }),
        _ => None,
    });
    let soft = p.get("softEdges").map(|v| v.as_f64().filter(|x| *x > 0.0).map(|x| x.min(100.0)));
    let refl = str_param(p, "reflection").map(|r| match r {
        "none" => None,
        "half" => Some(Reflection { blur: 0.5, start_alpha: 0.5, end_alpha: 0.0, end_pos: 0.55, dist: 4.0 }),
        "full" => Some(Reflection { blur: 0.5, start_alpha: 0.5, end_alpha: 0.0, end_pos: 0.9, dist: 4.0 }),
        _ => Some(Reflection { blur: 0.5, start_alpha: 0.5, end_alpha: 0.0, end_pos: 0.35, dist: 0.0 }),
    });
    let bevel = str_param(p, "bevel").map(|b| if b == "none" { None } else { Some(b.to_string()) });
    edit_shapes(s, p, "shape.effects", |sh| {
        if reset {
            sh.effects = None;
            return Ok(());
        }
        let e = sh.effects.get_or_insert_with(Effects::default);
        if let Some(sw) = &shadow {
            match sw {
                Some(x) if x.inner => {
                    e.inner_shadow = Some(x.clone());
                    e.outer_shadow = None;
                }
                Some(x) => {
                    e.outer_shadow = Some(x.clone());
                    e.inner_shadow = None;
                }
                None => {
                    e.outer_shadow = None;
                    e.inner_shadow = None;
                }
            }
        }
        if let Some(g) = &glow {
            e.glow.clone_from(g);
        }
        if let Some(sf) = soft {
            e.soft_edge = sf;
        }
        if let Some(r) = &refl {
            e.reflection.clone_from(r);
        }
        if let Some(b) = &bevel {
            e.bevel.clone_from(b);
        }
        Ok(())
    })
}

pub(super) fn quick_style(s: &mut Session, p: &Value) -> Result<Value> {
    let i = usize_param(p, "index").unwrap_or(1).min(41);
    let (st, f, l) = quick_style_for(i);
    edit_shapes(s, p, "shape.quickStyle", |sh| {
        sh.style = Some(st.clone());
        sh.fill.clone_from(&f);
        sh.line.clone_from(&l);
        sh.effects = None;
        if let Some(t) = sh.text.as_mut() {
            deckcraft_model::edit::format_all(t, &|r| r.fill = None);
        }
        Ok(())
    })
}
