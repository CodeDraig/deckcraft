//! Master editing targets, layouts and placeholder commands.

use super::super::*;
use super::master_index;
use crate::{Result, Session, Target};
use deckcraft_model::text::TextBody;
use deckcraft_model::{PhType, Shape, ShapeId};
use serde_json::{Value, json};
use std::sync::Arc;

pub(super) fn master_view(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let mi = usize_param(p, "master").unwrap_or_else(|| master_index(&st.doc, &st.selection));
    let layout = usize_param(p, "layout")
        .or_else(|| st.current_slide().and_then(|sl| st.doc.masters.get(mi).and_then(|m| m.layouts.iter().position(|l| l.id == sl.layout))));
    let target = match layout {
        Some(l) => Target::Layout { master: mi, layout: l },
        None => Target::Master { master: mi },
    };
    s.select(|_, sel| {
        sel.target = target;
        sel.shapes.clear();
        sel.text = None;
    })?;
    Ok(serde_json::to_value(target).unwrap_or_default())
}

pub(super) fn close_master(s: &mut Session, _p: &Value) -> Result<Value> {
    s.select(|_, sel| {
        sel.target = Target::Slides;
        sel.shapes.clear();
        sel.text = None;
    })?;
    ok()
}

pub(super) fn insert_layout(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").unwrap_or("Custom Layout").to_string();
    s.edit(|doc, sel| {
        let mi = master_index(doc, sel);
        let id = deckcraft_model::LayoutId(doc.alloc_id());
        let tid = ShapeId(doc.alloc_id());
        let size = doc.slide_size;
        let m = Arc::make_mut(doc.masters.get_mut(mi).ok_or_else(|| bad("master.insertLayout", "no master"))?);
        let title = m.shapes.iter().find(|x| x.ph_type() == Some(PhType::Title)).cloned().map(|mut t| {
            t.id = tid;
            t.text = Some(TextBody::default());
            t
        });
        let _ = size;
        let at = match sel.target {
            Target::Layout { layout, .. } => layout + 1,
            _ => m.layouts.len(),
        };
        m.layouts.insert(
            at.min(m.layouts.len()),
            deckcraft_model::Layout {
                id,
                name,
                kind: deckcraft_model::LayoutType::Custom,
                shapes: title.into_iter().collect(),
                background: None,
                show_master_shapes: true,
                preserve: true,
                raw_ext: None,
            },
        );
        sel.target = Target::Layout { master: mi, layout: at.min(m.layouts.len().saturating_sub(1)) };
        Ok(json!({"id": id}))
    })
}

pub(super) fn layout_target(s: &Session, p: &Value) -> Result<(usize, usize)> {
    let st = s.doc()?;
    let (m, l) = match st.selection.target {
        Target::Layout { master, layout } => (master, layout),
        _ => (master_index(&st.doc, &st.selection), 0),
    };
    Ok((usize_param(p, "master").unwrap_or(m), usize_param(p, "layout").unwrap_or(l)))
}

pub(super) fn rename_layout(s: &mut Session, p: &Value) -> Result<Value> {
    let (mi, li) = layout_target(s, p)?;
    let name = str_param(p, "name").ok_or_else(|| bad("master.renameLayout", "missing `name`"))?.to_string();
    s.edit(|doc, _| {
        let l =
            doc.masters.get_mut(mi).and_then(|m| Arc::make_mut(m).layouts.get_mut(li)).ok_or_else(|| bad("master.renameLayout", "no such layout"))?;
        l.name = name;
        Ok(())
    })?;
    ok()
}

pub(super) fn delete_layout(s: &mut Session, p: &Value) -> Result<Value> {
    let (mi, li) = layout_target(s, p)?;
    s.edit(|doc, sel| {
        let id = doc.masters.get(mi).and_then(|m| m.layouts.get(li)).map(|l| l.id).ok_or_else(|| bad("master.deleteLayout", "no such layout"))?;
        if doc.slides.iter().any(|x| x.layout == id) {
            return Err(bad("master.deleteLayout", "slides use this layout"));
        }
        let m = Arc::make_mut(doc.masters.get_mut(mi).ok_or_else(|| bad("master.deleteLayout", "no master"))?);
        if m.layouts.len() <= 1 {
            return Err(bad("master.deleteLayout", "a master needs at least one layout"));
        }
        m.layouts.remove(li);
        sel.target = Target::Layout { master: mi, layout: li.min(m.layouts.len().saturating_sub(1)) };
        Ok(())
    })?;
    ok()
}

pub(super) fn insert_placeholder(s: &mut Session, p: &Value) -> Result<Value> {
    let kind = match str_param(p, "kind").unwrap_or("content") {
        "text" => PhType::Body,
        "picture" => PhType::Picture,
        "chart" => PhType::Chart,
        "table" => PhType::Table,
        "media" => PhType::Media,
        _ => PhType::Obj,
    };
    if !matches!(s.doc()?.selection.target, Target::Layout { .. }) {
        return Err(bad("master.insertPlaceholder", "select a layout in Slide Master view"));
    }
    let size = s.doc()?.doc.slide_size;
    let rect = rect_param(p, "rect").unwrap_or(deckcraft_geom::Xfrm::new(size.width * 0.25, size.height * 0.3, size.width * 0.5, size.height * 0.4));
    let idx = s.doc()?.shapes().iter().filter_map(|x| x.ph.as_ref().map(|p| p.idx)).max().unwrap_or(0) + 1;
    let shape = Shape {
        xfrm: Some(rect),
        ph: Some(deckcraft_model::Placeholder { kind, idx, ..Default::default() }),
        text: Some(TextBody::default()),
        ..Default::default()
    };
    let id = super::super::insert::add_shape(s, shape, true)?;
    Ok(json!({"id": id}))
}
