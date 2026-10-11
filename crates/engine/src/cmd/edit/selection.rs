//! Document selection commands.

use super::super::*;
use crate::{Result, Session};
use deckcraft_model::{Shape, ShapeId};
use serde_json::{Value, json};

pub(super) fn count(v: &[Shape]) -> usize {
    let mut n = 0;
    deckcraft_model::walk(v, &mut |_, _| n += 1);
    n
}

pub(super) fn select_all(s: &mut Session, _p: &Value) -> Result<Value> {
    if s.doc()?.selection.text.is_some() {
        return s.execute("text.selectAll", &json!({}));
    }
    s.select(|_, sel| {
        sel.text = None;
    })?;
    let ids: Vec<ShapeId> = s.doc()?.shapes().iter().filter(|x| !x.hidden && !x.locked).map(|x| x.id).collect();
    s.select(|_, sel| sel.shapes = ids.clone())?;
    Ok(json!({"ids": ids}))
}

pub(super) fn deselect(s: &mut Session, _p: &Value) -> Result<Value> {
    s.select(|_, sel| {
        if let Some(t) = sel.text.take() {
            // Esc while editing text selects the shape.
            if !t.notes {
                sel.shapes = vec![t.shape];
            }
        } else {
            sel.shapes.clear();
            sel.cells = None;
        }
    })?;
    ok()
}

pub(super) fn select(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = ids_param(p, "ids").or_else(|| id_param(p, "id").map(|i| vec![i])).unwrap_or_default();
    let add = bool_or(p, "add", false);
    let toggle = bool_or(p, "toggle", false);
    let valid: Vec<ShapeId> = ids.into_iter().filter(|i| s.active().is_some_and(|d| d.shape(*i).is_some())).collect();
    s.select(|_, sel| {
        sel.text = None;
        sel.cells = None;
        if toggle {
            for i in &valid {
                if let Some(k) = sel.shapes.iter().position(|x| x == i) {
                    sel.shapes.remove(k);
                } else {
                    sel.shapes.push(*i);
                }
            }
        } else if add {
            for i in &valid {
                if !sel.shapes.contains(i) {
                    sel.shapes.push(*i);
                }
            }
        } else {
            sel.shapes = valid.clone();
        }
    })?;
    Ok(json!({"selected": s.doc()?.selection.shapes}))
}
