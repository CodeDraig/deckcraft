//! Read presentation settings and core metadata.

use super::dml;
use crate::xml::El;
use deckcraft_model::{HeaderFooter, PhType, Presentation, Properties, ShowSettings};
use std::collections::HashMap;

pub(super) fn read_show(sp: &El, shows: &HashMap<String, String>) -> ShowSettings {
    let mut s = ShowSettings {
        loop_until_esc: sp.bool("loop").unwrap_or(false),
        without_narration: sp.bool("showNarration") == Some(false),
        without_animation: sp.bool("showAnimation") == Some(false),
        use_timings: sp.bool("useTimings").unwrap_or(true),
        ..Default::default()
    };
    if sp.child("browse").is_some() {
        s.show_type = "browsed".into();
    } else if sp.child("kiosk").is_some() {
        s.show_type = "kiosk".into();
    }
    if let Some(r) = sp.child("sldRg") {
        s.range = Some((r.u32("st").unwrap_or(1).max(1), r.u32("end").unwrap_or(1).max(1)));
    }
    if let Some(c) = sp.child("custShow") {
        let id = c.attr("id").unwrap_or("");
        s.custom_show = Some(shows.get(id).cloned().unwrap_or_else(|| id.to_string()));
    }
    if let Some(c) = sp.child("penClr").and_then(dml::color)
        && let deckcraft_model::ColorBase::Rgb { rgb } = c.base
    {
        s.pen_color = rgb;
    }
    s
}

pub(super) fn read_core(root: &El, p: &mut Properties) {
    for e in root.elements() {
        let t = e.text();
        match e.local() {
            "title" => p.title = t,
            "subject" => p.subject = t,
            "creator" => p.author = t,
            "keywords" => p.keywords = t,
            "description" => p.comments = t,
            "category" => p.category = t,
            "lastModifiedBy" => p.last_modified_by = t,
            "revision" => p.revision = t.trim().parse().unwrap_or(0),
            "created" => p.created = t,
            "modified" => p.modified = t,
            _ => {}
        }
    }
}

/// Which footer placeholders the slides show.
pub(super) fn header_footer(p: &Presentation) -> HeaderFooter {
    let mut hf = HeaderFooter::default();
    for s in &p.slides {
        for sh in &s.shapes {
            match sh.ph_type() {
                Some(PhType::SlideNum) => hf.slide_number = true,
                Some(PhType::Date) => {
                    hf.date = true;
                    if let Some(t) = &sh.text
                        && !t.paragraphs.iter().flat_map(|p| &p.runs).any(|r| matches!(r.kind, deckcraft_model::text::RunKind::Field { .. }))
                        && hf.date_text.is_empty()
                    {
                        hf.date_text = t.text();
                    }
                }
                Some(PhType::Footer) => {
                    hf.footer = true;
                    if hf.footer_text.is_empty()
                        && let Some(t) = &sh.text
                    {
                        hf.footer_text = t.text();
                    }
                }
                _ => {}
            }
        }
    }
    hf
}
