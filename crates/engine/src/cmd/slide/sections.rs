//! Slide-section membership and ordering commands.

use super::super::*;
use crate::{Result, Session};
use deckcraft_model::{Section, SlideId};
use serde_json::{Value, json};

pub(super) fn section_add(s: &mut Session, p: &Value) -> Result<Value> {
    let at = usize_param(p, "at").unwrap_or(s.doc()?.selection.slide);
    let name = str_param(p, "name").unwrap_or("Untitled Section").to_string();
    s.edit(|doc, _| {
        let at = at.min(doc.slides.len().saturating_sub(1));
        let ids: Vec<SlideId> = doc.slides.iter().map(|x| x.id).collect();
        if doc.sections.is_empty() {
            // The slides before `at` go into a "Default Section".
            if at > 0 {
                doc.sections.push(Section { name: "Default Section".into(), slides: ids.get(..at).map(|x| x.to_vec()).unwrap_or_default() });
            }
            doc.sections.push(Section { name, slides: ids.get(at..).map(|x| x.to_vec()).unwrap_or_default() });
        } else {
            let Some(start) = ids.get(at).copied() else { return Err(bad("section.add", "no slide")) };
            let si = doc.section_of(start).unwrap_or(0);
            let sec = doc.sections.get_mut(si).ok_or_else(|| bad("section.add", "no section"))?;
            let k = sec.slides.iter().position(|x| *x == start).unwrap_or(sec.slides.len());
            let tail = sec.slides.split_off(k);
            doc.sections.insert(si + 1, Section { name, slides: tail });
            doc.sections.retain(|x| !x.slides.is_empty() || x.name != "Default Section");
        }
        Ok(json!({"sections": doc.sections.len()}))
    })
}

pub(super) fn section_rename(s: &mut Session, p: &Value) -> Result<Value> {
    let i = usize_param(p, "index").ok_or_else(|| bad("section.rename", "missing `index`"))?;
    let name = str_param(p, "name").unwrap_or("").to_string();
    s.edit(|doc, _| {
        let sec = doc.sections.get_mut(i).ok_or_else(|| bad("section.rename", "no such section"))?;
        sec.name = name;
        Ok(())
    })?;
    ok()
}

pub(super) fn section_remove(s: &mut Session, p: &Value) -> Result<Value> {
    let i = usize_param(p, "index").ok_or_else(|| bad("section.remove", "missing `index`"))?;
    let with_slides = bool_or(p, "slides", false);
    s.edit(|doc, sel| {
        if i >= doc.sections.len() {
            return Err(bad("section.remove", "no such section"));
        }
        let sec = doc.sections.remove(i);
        if with_slides {
            doc.slides.retain(|x| !sec.slides.contains(&x.id));
            sel.slide = sel.slide.min(doc.slides.len().saturating_sub(1));
        } else if let Some(prev) = i.checked_sub(1).and_then(|k| doc.sections.get_mut(k)) {
            prev.slides.extend(sec.slides);
        } else if let Some(next) = doc.sections.get_mut(0) {
            let mut v = sec.slides;
            v.append(&mut next.slides);
            next.slides = v;
        }
        Ok(())
    })?;
    ok()
}

pub(super) fn section_remove_all(s: &mut Session, _p: &Value) -> Result<Value> {
    s.edit(|doc, _| {
        doc.sections.clear();
        Ok(())
    })?;
    ok()
}

pub(super) fn section_move(s: &mut Session, p: &Value) -> Result<Value> {
    let i = usize_param(p, "index").ok_or_else(|| bad("section.move", "missing `index`"))?;
    let to = usize_param(p, "to").ok_or_else(|| bad("section.move", "missing `to`"))?;
    s.edit(|doc, _| {
        if i >= doc.sections.len() {
            return Err(bad("section.move", "no such section"));
        }
        let sec = doc.sections.remove(i);
        let to = to.min(doc.sections.len());
        doc.sections.insert(to, sec);
        // Reorder slides to follow sections.
        let order: Vec<SlideId> = doc.sections.iter().flat_map(|x| x.slides.iter().copied()).collect();
        doc.slides.sort_by_key(|x| order.iter().position(|o| *o == x.id).unwrap_or(usize::MAX));
        Ok(())
    })?;
    ok()
}
