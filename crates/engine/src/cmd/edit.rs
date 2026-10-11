//! Edit menu: undo/redo, clipboard, duplicate, delete, select all.

mod selection;
use selection::{count, deselect, select, select_all};

mod clipboard;
pub(crate) use clipboard::reid;
use clipboard::{copy, cut, duplicate, paste, paste_text, slides_scope};

use std::sync::Arc;

use deckcraft_model::{Shape, ShapeId};
use serde_json::{Value, json};

use super::*;
use crate::{HistoryEntry, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "edit.undo", "Undo", ["Edit"], Some("Cmd+Z"), "{}", can_undo, undo),
        cmd!(noundo "edit.redo", "Redo", ["Edit"], Some("Cmd+Y"), "{}", can_redo, redo),
        cmd!("edit.cut", "Cut", ["Home", "Clipboard"], Some("Cmd+X"), "{scope?: slides}", has_doc, cut),
        cmd!(noundo "edit.copy", "Copy", ["Home", "Clipboard"], Some("Cmd+C"), "{scope?: slides} → {text}", has_doc, copy),
        cmd!("edit.paste", "Paste", ["Home", "Clipboard"], Some("Cmd+V"), "{text?: plain text to paste, scope?: slides}", has_doc, paste),
        cmd!("edit.pasteText", "Paste and Match Formatting", ["Edit"], Some("Cmd+Shift+V"), "{text?}", has_doc, paste_text),
        cmd!("edit.duplicate", "Duplicate", ["Edit"], Some("Cmd+D"), "{ids?}", has_selection, duplicate),
        cmd!("edit.delete", "Delete", ["Edit"], Some("Delete"), "{ids?, scope?: slides}", has_doc, delete),
        cmd!(noundo "edit.selectAll", "Select All", ["Edit"], Some("Cmd+A"), "{}", has_slide, select_all),
        cmd!(noundo "edit.deselect", "Deselect", [], Some("Escape"), "{}", has_doc, deselect),
        cmd!(noundo "edit.select", "Select Objects", [], None, "{ids: [id], add?: bool, toggle?: bool}", has_slide, select),
    ]
}

fn restore(s: &mut Session, redo: bool) -> Result<Value> {
    let st = s.doc_mut()?;
    let (from, to) = if redo { (&mut st.history.redo, &mut st.history.undo) } else { (&mut st.history.undo, &mut st.history.redo) };
    let Some(e) = from.pop() else { return ok() };
    to.push(HistoryEntry { label: e.label.clone(), doc: st.doc.clone(), selection: st.selection.clone() });
    st.doc = e.doc;
    st.selection = e.selection;
    st.interaction = None;
    st.revision += 1;
    Ok(json!({"label": e.label}))
}

fn undo(s: &mut Session, _p: &Value) -> Result<Value> {
    restore(s, false)
}
fn redo(s: &mut Session, _p: &Value) -> Result<Value> {
    restore(s, true)
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    if s.doc()?.selection.text.is_some() && ids_param(p, "ids").is_none() {
        return super::text::delete_dir(s, "forward");
    }
    if slides_scope(s, p) && ids_param(p, "ids").is_none() {
        return s.execute("slide.delete", &json!({}));
    }
    let ids = targets(s, p)?;
    if ids.is_empty() {
        return ok();
    }
    s.edit(|doc, sel| {
        let shapes = crate::shapes_mut(doc, sel).ok_or_else(|| bad("edit.delete", "no slide"))?;
        let n = remove_shapes(shapes, &ids);
        // Animations of deleted shapes go too.
        if sel.target == crate::Target::Slides
            && let Some(sl) = doc.slides.get_mut(sel.slide)
        {
            Arc::make_mut(sl).animations.retain(|a| !ids.contains(&a.shape));
        }
        sel.shapes.retain(|i| !ids.contains(i));
        sel.text = None;
        Ok(json!({"deleted": n}))
    })
}

/// Remove shapes by id anywhere in the tree; empty groups go too.
pub(crate) fn remove_shapes(shapes: &mut Vec<Shape>, ids: &[ShapeId]) -> usize {
    let before = count(shapes);
    fn rec(v: &mut Vec<Shape>, ids: &[ShapeId], depth: usize) {
        v.retain(|s| !ids.contains(&s.id));
        if depth > 64 {
            return;
        }
        for s in v.iter_mut() {
            if let Some(ch) = s.children_mut() {
                rec(ch, ids, depth + 1);
            }
        }
        v.retain(|s| !s.is_group() || !s.children().is_empty());
    }
    rec(shapes, ids, 0);
    before.saturating_sub(count(shapes))
}
