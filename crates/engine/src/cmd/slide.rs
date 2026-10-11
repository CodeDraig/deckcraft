//! Slides: new, duplicate, delete, move, layout, reset, hide, notes, sections, navigation.

mod navigation;
use navigation::{first, go, last, next, prev, select_slides};

mod layout;
pub use layout::{apply_layout, find_layout};
use layout::{current_master, reset, set_layout};

mod sections;
use sections::{section_add, section_move, section_remove, section_remove_all, section_rename};

use std::sync::Arc;

use deckcraft_model::text::TextBody;
use deckcraft_model::{LayoutType, Presentation, SlideId, defaults};
use serde_json::{Value, json};

use super::*;
use crate::{Result, Session, Target};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "slide.new",
            "New Slide",
            ["Home", "Slides"],
            Some("Cmd+Shift+N"),
            "{layout?: layout name or kind (title, titleAndContent, sectionHeader, twoContent, comparison, titleOnly, blank, contentWithCaption, pictureWithCaption), at?: index, title?: text, body?: text}",
            has_doc,
            new
        ),
        cmd!("slide.duplicate", "Duplicate Slide", ["Insert"], None, "{index?}", has_slide, duplicate),
        cmd!("slide.delete", "Delete Slide", ["Edit"], None, "{index?, ids?: [slide id]}", has_slide, delete),
        cmd!("slide.move", "Move Slide", [], None, "{from?: index, to: index}", has_slide, move_slide),
        cmd!(noundo "slide.go", "Go to Slide", [], None, "{index} | {id}", has_slide, go),
        cmd!(noundo "slide.next", "Next Slide", [], Some("PageDown"), "{}", has_slide, next),
        cmd!(noundo "slide.previous", "Previous Slide", [], Some("PageUp"), "{}", has_slide, prev),
        cmd!(noundo "slide.first", "First Slide", [], Some("Home"), "{}", has_slide, first),
        cmd!(noundo "slide.last", "Last Slide", [], Some("End"), "{}", has_slide, last),
        cmd!(noundo "slide.selectSlides", "Select Slides", [], None, "{ids?: [slide id], indices?: [index], add?: bool}", has_slide, select_slides),
        cmd!("slide.hide", "Hide Slide", ["Slide Show", "Set Up"], None, "{index?, hidden?: bool}", has_slide, hide),
        cmd!("slide.layout", "Layout", ["Home", "Slides"], None, "{layout: name or kind, index?}", has_slide, set_layout),
        cmd!("slide.reset", "Reset", ["Home", "Slides"], None, "{index?}", has_slide, reset),
        cmd!("slide.notes", "Notes", ["View"], None, "{text, index?}", has_slide, notes),
        cmd!("slide.rename", "Rename Slide", [], None, "{name, index?}", has_slide, rename),
        cmd!("section.add", "Add Section", ["Home", "Slides", "Section"], None, "{name?, at?: slide index}", has_slide, section_add),
        cmd!("section.rename", "Rename Section", ["Home", "Slides", "Section"], None, "{index, name}", has_slide, section_rename),
        cmd!(
            "section.remove",
            "Remove Section",
            ["Home", "Slides", "Section"],
            None,
            "{index, slides?: bool (also delete its slides)}",
            has_slide,
            section_remove
        ),
        cmd!("section.removeAll", "Remove All Sections", ["Home", "Slides", "Section"], None, "{}", has_slide, section_remove_all),
        cmd!("section.move", "Move Section", [], None, "{index, to}", has_slide, section_move),
        cmd!("slide.fromOutline", "Slides from Outline…", ["Insert", "Slides From"], None, "{text}", has_doc, from_outline),
    ]
}

fn new(s: &mut Session, p: &Value) -> Result<Value> {
    s.edit(|doc, sel| {
        let master = current_master(doc, sel);
        let layout = match p.get("layout") {
            Some(k) => find_layout(doc, k, master).ok_or_else(|| bad("slide.new", format!("no layout {k}")))?,
            None => {
                // PowerPoint picks Title and Content after a title slide, else the current layout.
                let cur = doc.slides.get(sel.slide).map(|x| x.layout);
                let is_title = cur.and_then(|l| doc.layout(l)).is_some_and(|(_, l)| l.kind == LayoutType::Title);
                match cur {
                    Some(l) if !is_title => l,
                    _ => defaults::layout_of_kind(doc, LayoutType::TitleAndContent).or(cur).unwrap_or_default(),
                }
            }
        };
        let mut slide = defaults::new_slide(doc, layout);
        if let Some(t) = str_param(p, "title")
            && let Some(sh) = slide.shapes.iter_mut().find(|x| x.ph_type().is_some_and(|k| k.is_title()))
        {
            sh.text = Some(TextBody::from_text(t));
        }
        if let Some(b) = str_param(p, "body")
            && let Some(sh) = slide.shapes.iter_mut().find(|x| x.ph_type().is_some_and(|k| !k.is_title()))
        {
            sh.text = Some(TextBody::from_text(b));
        }
        let at = usize_param(p, "at").unwrap_or(if doc.slides.is_empty() { 0 } else { sel.slide + 1 }).min(doc.slides.len());
        let id = slide.id;
        // Inherit the section of the slide before.
        let section = at.checked_sub(1).and_then(|i| doc.slides.get(i)).and_then(|prev| doc.section_of(prev.id));
        doc.slides.insert(at, Arc::new(slide));
        if let Some(si) = section
            && let Some(sec) = doc.sections.get_mut(si)
        {
            let pos = at
                .checked_sub(1)
                .and_then(|i| doc.slides.get(i))
                .and_then(|prev| sec.slides.iter().position(|x| *x == prev.id))
                .map(|k| k + 1)
                .unwrap_or(sec.slides.len());
            sec.slides.insert(pos.min(sec.slides.len()), id);
        } else if let Some(first) = doc.sections.first_mut() {
            first.slides.insert(0, id);
        }
        sel.slide = at;
        sel.slides = vec![id];
        sel.shapes.clear();
        sel.text = None;
        sel.target = Target::Slides;
        Ok(json!({"index": at, "id": id}))
    })
}

fn index_of(s: &Session, p: &Value) -> Result<usize> {
    let st = s.doc()?;
    let i = usize_param(p, "index").unwrap_or(st.selection.slide);
    if i >= st.doc.slides.len() {
        return Err(bad("slide", format!("no slide {i}")));
    }
    Ok(i)
}

fn duplicate(s: &mut Session, p: &Value) -> Result<Value> {
    let indices: Vec<usize> = match usize_param(p, "index") {
        Some(i) => vec![i],
        None => {
            let st = s.doc()?;
            let mut v: Vec<usize> = st.selection.slides.iter().filter_map(|id| st.doc.slide_index(*id)).collect();
            if v.is_empty() {
                v.push(st.selection.slide);
            }
            v.sort_unstable();
            v
        }
    };
    s.edit(|doc, sel| {
        let last = *indices.last().unwrap_or(&0);
        let mut new_ids = vec![];
        for (k, i) in indices.iter().enumerate() {
            let Some(src) = doc.slides.get(*i).map(|x| x.as_ref().clone()) else { continue };
            let mut copy = src;
            copy.id = SlideId(doc.alloc_id());
            super::edit::reid(doc, &mut copy.shapes);
            // Animations follow the new shape ids by position.
            let old_ids: Vec<_> = doc
                .slides
                .get(*i)
                .map(|x| {
                    let mut v = vec![];
                    deckcraft_model::walk(&x.shapes, &mut |sh, _| v.push(sh.id));
                    v
                })
                .unwrap_or_default();
            let mut new_shape_ids = vec![];
            deckcraft_model::walk(&copy.shapes, &mut |sh, _| new_shape_ids.push(sh.id));
            for a in &mut copy.animations {
                if let Some(pos) = old_ids.iter().position(|x| *x == a.shape) {
                    a.shape = new_shape_ids.get(pos).copied().unwrap_or(a.shape);
                }
            }
            let section = doc.section_of(src_id(doc, *i));
            let at = (last + 1 + k).min(doc.slides.len());
            new_ids.push(copy.id);
            let nid = copy.id;
            doc.slides.insert(at, Arc::new(copy));
            if let Some(si) = section
                && let Some(sec) = doc.sections.get_mut(si)
            {
                sec.slides.push(nid);
            }
        }
        sel.slide = (last + new_ids.len()).min(doc.slides.len().saturating_sub(1));
        sel.slides = new_ids.clone();
        sel.shapes.clear();
        sel.text = None;
        Ok(json!({"ids": new_ids}))
    })
}

fn src_id(doc: &Presentation, i: usize) -> SlideId {
    doc.slides.get(i).map(|s| s.id).unwrap_or_default()
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    let ids: Vec<SlideId> = {
        let st = s.doc()?;
        if let Some(a) = p.get("ids").and_then(Value::as_array) {
            a.iter().filter_map(Value::as_u64).filter_map(|v| u32::try_from(v).ok()).map(SlideId).collect()
        } else if let Some(i) = usize_param(p, "index") {
            st.doc.slides.get(i).map(|x| vec![x.id]).unwrap_or_default()
        } else if !st.selection.slides.is_empty() {
            st.selection.slides.clone()
        } else {
            st.current_slide().map(|x| vec![x.id]).unwrap_or_default()
        }
    };
    if ids.is_empty() {
        return Err(bad("slide.delete", "no such slide"));
    }
    s.edit(|doc, sel| {
        let first = ids.iter().filter_map(|id| doc.slide_index(*id)).min().unwrap_or(0);
        doc.slides.retain(|x| !ids.contains(&x.id));
        for sec in &mut doc.sections {
            sec.slides.retain(|x| !ids.contains(x));
        }
        sel.slide = first.min(doc.slides.len().saturating_sub(1));
        sel.slides = doc.slides.get(sel.slide).map(|x| vec![x.id]).unwrap_or_default();
        sel.shapes.clear();
        sel.text = None;
        Ok(json!({"deleted": ids.len(), "remaining": doc.slides.len()}))
    })
}

fn move_slide(s: &mut Session, p: &Value) -> Result<Value> {
    let from = usize_param(p, "from").unwrap_or(s.doc()?.selection.slide);
    let to = usize_param(p, "to").ok_or_else(|| bad("slide.move", "missing `to`"))?;
    s.edit(|doc, sel| {
        if from >= doc.slides.len() {
            return Err(bad("slide.move", format!("no slide {from}")));
        }
        let sl = doc.slides.remove(from);
        let to = to.min(doc.slides.len());
        let id = sl.id;
        doc.slides.insert(to, sl);
        // Keep sections consistent with order: move the id into the section of its new neighbour.
        if !doc.sections.is_empty() {
            for sec in &mut doc.sections {
                sec.slides.retain(|x| *x != id);
            }
            let neighbour = to.checked_sub(1).and_then(|i| doc.slides.get(i)).map(|x| x.id);
            let si = neighbour.and_then(|n| doc.section_of(n)).unwrap_or(0);
            let order: Vec<SlideId> = doc.slides.iter().map(|x| x.id).collect();
            if let Some(sec) = doc.sections.get_mut(si) {
                sec.slides.push(id);
                sec.slides.sort_by_key(|x| order.iter().position(|o| o == x).unwrap_or(usize::MAX));
            }
        }
        sel.slide = to;
        sel.slides = vec![id];
        Ok(json!({"index": to}))
    })
}

fn hide(s: &mut Session, p: &Value) -> Result<Value> {
    let targets: Vec<usize> = match usize_param(p, "index") {
        Some(i) => vec![i],
        None => {
            let st = s.doc()?;
            let v: Vec<usize> = st.selection.slides.iter().filter_map(|id| st.doc.slide_index(*id)).collect();
            if v.is_empty() { vec![st.selection.slide] } else { v }
        }
    };
    s.edit(|doc, _| {
        let all_hidden = targets.iter().all(|i| doc.slides.get(*i).is_some_and(|x| x.hidden));
        let value = bool_param(p, "hidden").unwrap_or(!all_hidden);
        for i in &targets {
            if let Some(sl) = doc.slides.get_mut(*i) {
                Arc::make_mut(sl).hidden = value;
            }
        }
        Ok(json!({"hidden": value}))
    })
}

fn notes(s: &mut Session, p: &Value) -> Result<Value> {
    let i = index_of(s, p)?;
    let text = str_param(p, "text").unwrap_or("").to_string();
    s.edit(|doc, _| {
        if let Some(sl) = doc.slides.get_mut(i) {
            Arc::make_mut(sl).notes = TextBody::from_text(&text);
        }
        Ok(())
    })?;
    ok()
}

fn rename(s: &mut Session, p: &Value) -> Result<Value> {
    let i = index_of(s, p)?;
    let name = str_param(p, "name").unwrap_or("").to_string();
    s.edit(|doc, _| {
        if let Some(sl) = doc.slides.get_mut(i) {
            Arc::make_mut(sl).name = name;
        }
        Ok(())
    })?;
    ok()
}

fn from_outline(s: &mut Session, p: &Value) -> Result<Value> {
    let text = str_param(p, "text").unwrap_or("").to_string();
    s.edit(|doc, sel| {
        let before = doc.slides.len();
        let n = deckcraft_format::outline_to_slides(doc, &text);
        // outline_to_slides appends; move them after the current slide.
        let at = (sel.slide + 1).min(before);
        let new: Vec<_> = doc.slides.drain(before..).collect();
        for (k, sl) in new.into_iter().enumerate() {
            doc.slides.insert(at + k, sl);
        }
        Ok(json!({"slides": n}))
    })
}
