//! Slide layout selection and placeholder reset commands.

use super::super::*;
use super::index_of;
use crate::{Result, Session};
use deckcraft_model::text::TextBody;
use deckcraft_model::{LayoutId, LayoutType, Presentation};
use serde_json::{Value, json};
use std::sync::Arc;

/// Resolve a layout by id, name or kind keyword.
pub fn find_layout(doc: &Presentation, key: &Value, master: usize) -> Option<LayoutId> {
    if let Some(n) = key.as_u64() {
        let id = LayoutId(u32::try_from(n).ok()?);
        if doc.layout(id).is_some() {
            return Some(id);
        }
    }
    let s = key.as_str()?;
    let m = doc.masters.get(master).or(doc.masters.first())?;
    let norm = |x: &str| x.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase();
    let k = norm(s);
    let kind = match k.as_str() {
        "title" | "titleslide" => Some(LayoutType::Title),
        "titleandcontent" | "content" | "obj" => Some(LayoutType::TitleAndContent),
        "sectionheader" | "section" => Some(LayoutType::SectionHeader),
        "twocontent" | "two" => Some(LayoutType::TwoContent),
        "comparison" => Some(LayoutType::Comparison),
        "titleonly" => Some(LayoutType::TitleOnly),
        "blank" => Some(LayoutType::Blank),
        "contentwithcaption" => Some(LayoutType::ContentWithCaption),
        "picturewithcaption" | "picture" => Some(LayoutType::PictureWithCaption),
        "titleandverticaltext" => Some(LayoutType::TitleAndVerticalText),
        "verticaltitleandtext" => Some(LayoutType::VerticalTitleAndText),
        _ => None,
    };
    m.layouts.iter().find(|l| norm(&l.name) == k).or_else(|| kind.and_then(|kd| m.layouts.iter().find(|l| l.kind == kd))).map(|l| l.id)
}

pub(super) fn current_master(doc: &Presentation, sel: &crate::Selection) -> usize {
    doc.slides.get(sel.slide).and_then(|s| doc.masters.iter().position(|m| m.layout(s.layout).is_some())).unwrap_or(0)
}

/// Re-map a slide's placeholders onto a new layout (content kept, matched by type/index).
pub fn apply_layout(doc: &mut Presentation, index: usize, layout: LayoutId) -> Result<()> {
    let lay_shapes: Vec<deckcraft_model::Shape> =
        doc.layout(layout).map(|(_, l)| l.shapes.clone()).ok_or_else(|| bad("slide.layout", "no such layout"))?;
    let mut fresh = vec![];
    for ls in lay_shapes.iter().filter(|x| x.ph.as_ref().is_some_and(|p| !p.kind.is_footer_kind())) {
        let id = deckcraft_model::ShapeId(doc.alloc_id());
        fresh.push((id, ls.clone()));
    }
    let slide = doc.slides.get_mut(index).ok_or_else(|| bad("slide.layout", "no slide"))?;
    let slide = Arc::make_mut(slide);
    let mut old: Vec<deckcraft_model::Shape> = std::mem::take(&mut slide.shapes);
    let mut out = vec![];
    for (id, ls) in fresh {
        let Some(lph) = ls.ph.clone() else { continue };
        // Find matching old placeholder: same idx, else same title-ness/compatible type.
        let k = old
            .iter()
            .position(|o| o.ph.as_ref().is_some_and(|p| p.kind == lph.kind && (p.idx == lph.idx || lph.kind.is_title())))
            .or_else(|| old.iter().position(|o| o.ph.as_ref().is_some_and(|p| p.kind.is_title() == lph.kind.is_title() && !p.kind.is_footer_kind())));
        match k {
            Some(k) => {
                let mut o = old.remove(k);
                if let Some(p) = o.ph.as_mut() {
                    p.kind = lph.kind;
                    p.idx = lph.idx;
                }
                o.xfrm = None;
                out.push(o);
            }
            None => {
                out.push(deckcraft_model::Shape { id, name: ls.name.clone(), ph: Some(lph), text: Some(TextBody::default()), ..Default::default() })
            }
        }
    }
    // Leftover placeholders with content stay as they were (positioned); empty ones go.
    for o in old {
        let empty_ph = o.ph.is_some() && o.text.as_ref().is_none_or(|t| t.is_empty()) && matches!(o.kind, deckcraft_model::ShapeKind::Shape);
        if !empty_ph {
            out.push(o);
        }
    }
    slide.shapes = out;
    slide.layout = layout;
    Ok(())
}

pub(super) fn set_layout(s: &mut Session, p: &Value) -> Result<Value> {
    let key = p.get("layout").cloned().ok_or_else(|| bad("slide.layout", "missing `layout`"))?;
    let targets: Vec<usize> = match usize_param(p, "index") {
        Some(i) => vec![i],
        None => {
            let st = s.doc()?;
            let v: Vec<usize> = st.selection.slides.iter().filter_map(|id| st.doc.slide_index(*id)).collect();
            if v.is_empty() { vec![st.selection.slide] } else { v }
        }
    };
    s.edit(|doc, sel| {
        let master = current_master(doc, sel);
        let layout = find_layout(doc, &key, master).ok_or_else(|| bad("slide.layout", format!("no layout {key}")))?;
        for i in &targets {
            apply_layout(doc, *i, layout)?;
        }
        sel.shapes.clear();
        sel.text = None;
        Ok(json!({"layout": layout}))
    })
}

pub(super) fn reset(s: &mut Session, p: &Value) -> Result<Value> {
    let i = index_of(s, p)?;
    s.edit(|doc, sel| {
        let layout = doc.slides.get(i).map(|x| x.layout).unwrap_or_default();
        apply_layout(doc, i, layout)?;
        if let Some(sl) = doc.slides.get_mut(i) {
            let sl = Arc::make_mut(sl);
            sl.background = None;
            for sh in &mut sl.shapes {
                if sh.ph.is_some() {
                    sh.xfrm = None;
                    sh.fill = None;
                    sh.line = None;
                    sh.effects = None;
                    sh.geom = Default::default();
                    if let Some(t) = sh.text.as_mut() {
                        t.body = Default::default();
                        t.list_style = Default::default();
                        for para in &mut t.paragraphs {
                            para.props = Default::default();
                            for r in &mut para.runs {
                                r.props = Default::default();
                            }
                        }
                    }
                }
            }
        }
        sel.shapes.clear();
        Ok(())
    })?;
    ok()
}
