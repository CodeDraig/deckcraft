//! Slide navigation and multi-slide selection commands.

use super::super::*;
use crate::{Result, Session, Target};
use deckcraft_model::SlideId;
use serde_json::{Value, json};

pub(super) fn go_to(s: &mut Session, i: usize) -> Result<Value> {
    s.select(|doc, sel| {
        let n = doc.slides.len();
        sel.slide = i.min(n.saturating_sub(1));
        sel.slides = doc.slides.get(sel.slide).map(|x| vec![x.id]).unwrap_or_default();
        sel.shapes.clear();
        sel.text = None;
        sel.cells = None;
        sel.target = Target::Slides;
    })?;
    Ok(json!({"index": s.doc()?.selection.slide}))
}

pub(super) fn go(s: &mut Session, p: &Value) -> Result<Value> {
    let i = match (usize_param(p, "index"), p.get("id").and_then(Value::as_u64)) {
        (Some(i), _) => i,
        (None, Some(id)) => s.doc()?.doc.slide_index(SlideId(u32::try_from(id).unwrap_or(0))).ok_or_else(|| bad("slide.go", "no such slide"))?,
        _ => return Err(bad("slide.go", "missing `index`")),
    };
    go_to(s, i)
}

pub(super) fn next(s: &mut Session, _p: &Value) -> Result<Value> {
    let i = s.doc()?.selection.slide + 1;
    go_to(s, i)
}

pub(super) fn prev(s: &mut Session, _p: &Value) -> Result<Value> {
    let i = s.doc()?.selection.slide.saturating_sub(1);
    go_to(s, i)
}

pub(super) fn first(s: &mut Session, _p: &Value) -> Result<Value> {
    go_to(s, 0)
}

pub(super) fn last(s: &mut Session, _p: &Value) -> Result<Value> {
    let n = s.doc()?.doc.slides.len();
    go_to(s, n.saturating_sub(1))
}

pub(super) fn select_slides(s: &mut Session, p: &Value) -> Result<Value> {
    let add = bool_or(p, "add", false);
    let ids: Vec<SlideId> = {
        let st = s.doc()?;
        let mut v: Vec<SlideId> = p
            .get("ids")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_u64).filter_map(|x| u32::try_from(x).ok()).map(SlideId).collect())
            .unwrap_or_default();
        if let Some(a) = p.get("indices").and_then(Value::as_array) {
            v.extend(a.iter().filter_map(Value::as_u64).filter_map(|i| st.doc.slides.get(i as usize)).map(|x| x.id));
        }
        v
    };
    s.select(|doc, sel| {
        if add {
            for i in &ids {
                if !sel.slides.contains(i) {
                    sel.slides.push(*i);
                }
            }
        } else {
            sel.slides = ids.clone();
        }
        if let Some(last) = ids.last().and_then(|i| doc.slide_index(*i)) {
            sel.slide = last;
        }
        sel.shapes.clear();
        sel.text = None;
    })?;
    Ok(json!({"selected": s.doc()?.selection.slides}))
}
