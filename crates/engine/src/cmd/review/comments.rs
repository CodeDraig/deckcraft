//! Review comment thread editing commands.

use super::super::*;
use crate::{Result, Session};
use deckcraft_model::Comment;
use serde_json::{Value, json};
use std::sync::Arc;

pub(super) fn now() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let (y, m, d) = super::super::design::civil((secs / 86_400) as i64);
        let t = secs % 86_400;
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", t / 3600, t / 60 % 60, t % 60)
    }
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::new_0().to_iso_string().into()
    }
}

pub(super) fn add(s: &mut Session, p: &Value) -> Result<Value> {
    let text = str_param(p, "text").ok_or_else(|| bad("comment.add", "missing `text`"))?.to_string();
    let author = s.prefs.author.clone();
    let shape = id_param(p, "id").or_else(|| s.active().and_then(|d| d.selection.shapes.first().copied()));
    let (x, y) = (f64_or(p, "x", 10.0), f64_or(p, "y", 10.0));
    s.edit(|doc, sel| {
        let sl = Arc::make_mut(doc.slides.get_mut(sel.slide).ok_or_else(|| bad("comment.add", "no slide"))?);
        let initials: String = author.split_whitespace().filter_map(|w| w.chars().next()).collect();
        sl.comments.push(Comment { author: author.clone(), initials, text, date: now(), x, y, resolved: false, replies: vec![], shape });
        Ok(json!({"index": sl.comments.len() - 1}))
    })
}

pub(super) fn reply(s: &mut Session, p: &Value) -> Result<Value> {
    let i = usize_param(p, "index").ok_or_else(|| bad("comment.reply", "missing `index`"))?;
    let text = str_param(p, "text").unwrap_or("").to_string();
    let author = s.prefs.author.clone();
    s.edit(|doc, sel| {
        let sl = Arc::make_mut(doc.slides.get_mut(sel.slide).ok_or_else(|| bad("comment.reply", "no slide"))?);
        let c = sl.comments.get_mut(i).ok_or_else(|| bad("comment.reply", "no such comment"))?;
        c.replies.push(Comment { author, text, date: now(), ..Default::default() });
        Ok(())
    })?;
    ok()
}

pub(super) fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    let all = bool_or(p, "all", false);
    let i = usize_param(p, "index");
    s.edit(|doc, sel| {
        if all {
            for sl in &mut doc.slides {
                Arc::make_mut(sl).comments.clear();
            }
            return Ok(());
        }
        let sl = Arc::make_mut(doc.slides.get_mut(sel.slide).ok_or_else(|| bad("comment.delete", "no slide"))?);
        match i {
            Some(i) if i < sl.comments.len() => {
                sl.comments.remove(i);
            }
            None => sl.comments.clear(),
            _ => return Err(bad("comment.delete", "no such comment")),
        }
        Ok(())
    })?;
    ok()
}

pub(super) fn resolve(s: &mut Session, p: &Value) -> Result<Value> {
    let i = usize_param(p, "index").ok_or_else(|| bad("comment.resolve", "missing `index`"))?;
    let v = bool_or(p, "resolved", true);
    s.edit(|doc, sel| {
        let sl = Arc::make_mut(doc.slides.get_mut(sel.slide).ok_or_else(|| bad("comment.resolve", "no slide"))?);
        sl.comments.get_mut(i).ok_or_else(|| bad("comment.resolve", "no such comment"))?.resolved = v;
        Ok(())
    })?;
    ok()
}

pub(super) fn list(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let i = usize_param(p, "slide").unwrap_or(st.selection.slide);
    Ok(serde_json::to_value(st.doc.slides.get(i).map(|x| x.comments.clone()).unwrap_or_default()).unwrap_or_default())
}
