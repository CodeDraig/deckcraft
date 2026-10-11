//! Slide background, dimensions and footer placeholder commands.

use super::super::*;
use super::today;
use crate::{Result, Session, Target};
use deckcraft_geom::Size;
use deckcraft_model::text::{Run, RunKind, TextBody};
use deckcraft_model::{Background, Fill, PhType, Shape, ShapeId, defaults};
use serde_json::{Value, json};
use std::sync::Arc;

pub(super) fn background(s: &mut Session, p: &Value) -> Result<Value> {
    let bg: Option<Background> = if bool_or(p, "reset", false) {
        None
    } else if let Some(c) = color_param(p, "color") {
        Some(Background::Fill { fill: Fill::solid(c) })
    } else if let Some(i) = usize_param(p, "style") {
        // Background styles: 3 theme bg fills × 4 colours (bg1, bg2, tx1, tx2).
        let slots =
            [deckcraft_color::SchemeSlot::Bg1, deckcraft_color::SchemeSlot::Bg2, deckcraft_color::SchemeSlot::Tx2, deckcraft_color::SchemeSlot::Tx1];
        let i = i.clamp(1, 12) - 1;
        Some(Background::Ref {
            idx: 1001 + (i / 4) as u32,
            color: deckcraft_model::ColorRef::scheme(slots.get(i % 4).copied().unwrap_or(deckcraft_color::SchemeSlot::Bg1)),
        })
    } else if let Some(g) = p.get("gradient") {
        let mut g = super::super::shape::gradient_param(g).ok_or_else(|| bad("design.background", "gradient needs two or more `stops`"))?;
        g.rotate_with_shape = false;
        Some(Background::Fill { fill: Fill::Gradient(g) })
    } else if let Some(pt) = p.get("pattern") {
        Some(Background::Fill {
            fill: Fill::Pattern(deckcraft_model::style::PatternFill {
                preset: pt.get("preset").and_then(Value::as_str).unwrap_or("pct50").to_string(),
                fg: pt.get("fg").and_then(color_value).unwrap_or(deckcraft_model::ColorRef::scheme(deckcraft_color::SchemeSlot::Accent1)),
                bg: pt.get("bg").and_then(color_value).unwrap_or(deckcraft_model::ColorRef::scheme(deckcraft_color::SchemeSlot::Bg1)),
            }),
        })
    } else if p.get("picture").is_some() {
        let (name, bytes) =
            super::super::insert::media_bytes(&with_param(p, "data", p.get("picture").cloned().unwrap_or_default()), "design.background")?;
        let ct = super::super::insert::content_type(&name, &bytes);
        let media = s.edit(|doc, _| Ok(doc.add_media(&name, ct, bytes)))?;
        Some(Background::Fill { fill: Fill::Picture(deckcraft_model::style::PictureFill { media, ..Default::default() }) })
    } else {
        return Err(bad("design.background", "give `color`, `gradient`, `pattern`, `picture`, `style` or `reset`"));
    };
    let all = bool_or(p, "all", false);
    let index = usize_param(p, "index");
    s.edit(|doc, sel| {
        match sel.target {
            Target::Master { master } => {
                if let Some(m) = doc.masters.get_mut(master) {
                    Arc::make_mut(m).background = bg.clone();
                }
            }
            Target::Layout { master, layout } => {
                if let Some(l) = doc.masters.get_mut(master).and_then(|m| Arc::make_mut(m).layouts.get_mut(layout)) {
                    l.background = bg.clone();
                }
            }
            Target::Slides => {
                let list: Vec<usize> = if all { (0..doc.slides.len()).collect() } else { vec![index.unwrap_or(sel.slide)] };
                for i in list {
                    if let Some(sl) = doc.slides.get_mut(i) {
                        Arc::make_mut(sl).background = bg.clone();
                    }
                }
            }
        }
        Ok(())
    })?;
    ok()
}

pub(super) fn hide_bg_graphics(s: &mut Session, p: &Value) -> Result<Value> {
    s.edit(|doc, sel| {
        if let Some(sl) = doc.slides.get_mut(sel.slide) {
            let sl = Arc::make_mut(sl);
            sl.show_master_shapes = !bool_param(p, "hide").unwrap_or(sl.show_master_shapes);
        }
        Ok(())
    })?;
    ok()
}

pub(super) fn scale_shapes(shapes: &mut [Shape], sx: f64, sy: f64, k: f64, ox: f64, oy: f64) {
    for sh in shapes {
        if let Some(x) = sh.xfrm.as_mut() {
            x.x = x.x * sx + ox;
            x.y = x.y * sy + oy;
            x.w *= sx;
            x.h *= sy;
        }
        if let Some(t) = sh.text.as_mut() {
            deckcraft_model::edit::format_all(t, &|r| {
                if let Some(sz) = r.size.as_mut() {
                    *sz = (*sz * k).max(1.0);
                }
            });
        }
    }
}

pub(super) fn slide_size(s: &mut Session, p: &Value) -> Result<Value> {
    let size = if let Some(pre) = str_param(p, "preset") {
        let k = pre.to_ascii_lowercase();
        let found = defaults::SLIDE_SIZES.iter().find(|(l, _, _)| l.to_ascii_lowercase().starts_with(&k) || l.to_ascii_lowercase().contains(&k));
        let (_, w, h) = found.ok_or_else(|| bad("design.slideSize", format!("unknown preset `{pre}`")))?;
        Size::new(*w, *h)
    } else {
        Size::new(
            f64_param(p, "w").ok_or_else(|| bad("design.slideSize", "missing `w`"))?.clamp(72.0, 4032.0),
            f64_param(p, "h").ok_or_else(|| bad("design.slideSize", "missing `h`"))?.clamp(72.0, 4032.0),
        )
    };
    let mode = str_param(p, "scale").unwrap_or("ensureFit").to_string();
    s.edit(|doc, _| {
        let old = doc.slide_size;
        let (sx, sy) = (size.width / old.width.max(1.0), size.height / old.height.max(1.0));
        let (sx, sy, k, ox, oy) = match mode.as_str() {
            "none" => (1.0, 1.0, 1.0, 0.0, 0.0),
            "maximize" => {
                let k = sx.max(sy);
                (k, k, k, (size.width - old.width * k) / 2.0, (size.height - old.height * k) / 2.0)
            }
            _ => {
                let k = sx.min(sy);
                (k, k, k, (size.width - old.width * k) / 2.0, (size.height - old.height * k) / 2.0)
            }
        };
        // Masters and layouts stretch to the new size; slides scale by mode.
        let (msx, msy) = (size.width / old.width.max(1.0), size.height / old.height.max(1.0));
        for m in &mut doc.masters {
            let m = Arc::make_mut(m);
            scale_shapes(&mut m.shapes, msx, msy, 1.0, 0.0, 0.0);
            for l in &mut m.layouts {
                scale_shapes(&mut l.shapes, msx, msy, 1.0, 0.0, 0.0);
            }
        }
        for sl in &mut doc.slides {
            scale_shapes(&mut Arc::make_mut(sl).shapes, sx, sy, k, ox, oy);
        }
        doc.slide_size = size;
        Ok(())
    })?;
    Ok(json!({"w": size.width, "h": size.height}))
}

pub(super) fn header_footer(s: &mut Session, p: &Value) -> Result<Value> {
    s.edit(|doc, sel| {
        let hf = &mut doc.header_footer;
        if let Some(v) = bool_param(p, "date") {
            hf.date = v;
        }
        if let Some(v) = str_param(p, "dateText") {
            hf.date_text = v.into();
        }
        if let Some(v) = bool_param(p, "slideNumber") {
            hf.slide_number = v;
        }
        if let Some(v) = bool_param(p, "footer") {
            hf.footer = v;
        }
        if let Some(v) = str_param(p, "footerText") {
            hf.footer_text = v.into();
            hf.footer = hf.footer || !v.is_empty();
        }
        if let Some(v) = bool_param(p, "hideOnTitle") {
            hf.hide_on_title = v;
        }
        let hf = hf.clone();
        let all = bool_or(p, "all", true);
        let list: Vec<usize> = if all { (0..doc.slides.len()).collect() } else { vec![sel.slide] };
        for i in list {
            let Some(layout) = doc.slides.get(i).map(|x| x.layout) else { continue };
            let lay = doc.layout(layout).map(|(_, l)| l.clone());
            let is_title = lay.as_ref().is_some_and(|l| l.kind == deckcraft_model::LayoutType::Title);
            let want = |k: PhType| {
                !(hf.hide_on_title && is_title)
                    && match k {
                        PhType::Date => hf.date,
                        PhType::SlideNum => hf.slide_number,
                        PhType::Footer => hf.footer,
                        _ => false,
                    }
            };
            let mut new_ids = vec![];
            for _ in 0..3 {
                new_ids.push(ShapeId(doc.alloc_id()));
            }
            let Some(sl) = doc.slides.get_mut(i) else { continue };
            let sl = Arc::make_mut(sl);
            for (n, k) in [PhType::Date, PhType::Footer, PhType::SlideNum].into_iter().enumerate() {
                let has = sl.shapes.iter().position(|x| x.ph_type() == Some(k));
                match (has, want(k)) {
                    (Some(pos), false) => {
                        sl.shapes.remove(pos);
                    }
                    (None, true) => {
                        let lp = lay.as_ref().and_then(|l| l.shapes.iter().find(|x| x.ph_type() == Some(k)).cloned());
                        let Some(lp) = lp else { continue };
                        let body = match k {
                            PhType::SlideNum => TextBody {
                                paragraphs: vec![deckcraft_model::Paragraph {
                                    runs: vec![Run {
                                        text: "‹#›".into(),
                                        props: Default::default(),
                                        kind: RunKind::Field { field: "slidenum".into() },
                                    }],
                                    ..Default::default()
                                }],
                                ..Default::default()
                            },
                            PhType::Date if hf.date_text.is_empty() => TextBody {
                                paragraphs: vec![deckcraft_model::Paragraph {
                                    runs: vec![Run { text: today(), props: Default::default(), kind: RunKind::Field { field: "datetime1".into() } }],
                                    ..Default::default()
                                }],
                                ..Default::default()
                            },
                            PhType::Date => TextBody::from_text(&hf.date_text),
                            _ => TextBody::from_text(&hf.footer_text),
                        };
                        sl.shapes.push(Shape {
                            id: new_ids.get(n).copied().unwrap_or_default(),
                            name: lp.name.clone(),
                            ph: lp.ph.clone(),
                            text: Some(body),
                            ..Default::default()
                        });
                    }
                    (Some(pos), true) if k == PhType::Footer => {
                        if let Some(sh) = sl.shapes.get_mut(pos) {
                            sh.text = Some(TextBody::from_text(&hf.footer_text));
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    })?;
    ok()
}
