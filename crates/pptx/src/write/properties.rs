//! Write presentation settings, slide ordering and package metadata.

use super::{Exp, Out, dml, ext_list, root_open};
use crate::opc::{self, NS_P14};
use crate::xml::{A, W};
use deckcraft_model::{Presentation, defaults};

pub(super) fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 10
        && b.iter().take(4).all(|c| c.is_ascii_digit())
        && b.get(4) == Some(&b'-')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || "-:.+TZ".contains(c))
}

/// Sections with every slide in exactly one section, in slide order.
pub(super) fn section_slides(p: &Presentation) -> Vec<(String, Vec<usize>)> {
    let mut out: Vec<(String, Vec<usize>)> = p.sections.iter().map(|s| (s.name.clone(), vec![])).collect();
    if out.is_empty() {
        return out;
    }
    let mut cur = 0usize;
    for (i, s) in p.slides.iter().enumerate() {
        if let Some(k) = p.sections.iter().position(|sec| sec.slides.contains(&s.id))
            && k >= cur
        {
            cur = k;
        }
        if let Some(sec) = out.get_mut(cur) {
            sec.1.push(i);
        }
    }
    out
}

pub(super) fn presentation_xml(x: &mut Exp, o: &mut Out, masters: &[(u64, String)], notes: Option<&str>, slides: &[String]) -> Vec<u8> {
    let p = x.p;
    let mut w = W::new();
    let first = (p.first_slide_number != 1).then_some(p.first_slide_number.min(9999));
    root_open(&mut w, "p:presentation", A::new().a("saveSubsetFonts", 1).o("firstSlideNum", first));
    w.open0("p:sldMasterIdLst");
    for (id, rid) in masters {
        w.empty("p:sldMasterId", A::new().a("id", id).a("r:id", rid));
    }
    w.close("p:sldMasterIdLst");
    if let Some(n) = notes {
        w.open0("p:notesMasterIdLst");
        w.empty("p:notesMasterId", A::new().a("r:id", n));
        w.close("p:notesMasterIdLst");
    }
    if !slides.is_empty() {
        w.open0("p:sldIdLst");
        for (i, rid) in slides.iter().enumerate() {
            w.empty("p:sldId", A::new().a("id", 256 + i).a("r:id", rid));
        }
        w.close("p:sldIdLst");
    }
    let clamp = |v: f64| dml::emu(v).clamp(914_400, 51_206_400);
    w.empty("p:sldSz", A::new().a("cx", clamp(p.slide_size.width)).a("cy", clamp(p.slide_size.height)));
    w.empty("p:notesSz", A::new().a("cx", dml::emu_pos(p.notes_size.width).max(1)).a("cy", dml::emu_pos(p.notes_size.height).max(1)));
    if !p.custom_shows.is_empty() {
        w.open0("p:custShowLst");
        for (i, cs) in p.custom_shows.iter().enumerate() {
            w.open("p:custShow", A::new().a("name", &cs.name).a("id", i));
            w.open0("p:sldLst");
            for sid in &cs.slides {
                if let Some(part) = x.slide_parts.get(sid).cloned() {
                    let rid = o.rels.add("slide", &opc::relative("ppt/presentation.xml", &part), false);
                    w.empty("p:sld", A::new().a("r:id", rid));
                }
            }
            w.close("p:sldLst");
            w.close("p:custShow");
        }
        w.close("p:custShowLst");
    }
    let dts = if p.default_text_style.is_empty() { defaults::default_text_style() } else { p.default_text_style.clone() };
    dml::list_style(&mut w, x, o, "p:defaultTextStyle", &dts);
    let sections = section_slides(p);
    let raw = p.raw_ext.as_deref().filter(|r| !r.trim().is_empty());
    if !sections.is_empty() || raw.is_some() {
        let mut inner = W::frag();
        if !sections.is_empty() {
            inner.open("p:ext", A::new().a("uri", crate::SECTION_EXT_URI));
            inner.open("p14:sectionLst", A::new().a("xmlns:p14", NS_P14));
            for (i, (name, list)) in sections.iter().enumerate() {
                inner.open("p14:section", A::new().a("name", name).a("id", crate::tables::guid(&format!("section:{i}:{name}"))));
                inner.open0("p14:sldIdLst");
                for si in list {
                    inner.empty("p14:sldId", A::new().a("id", 256 + si));
                }
                inner.close("p14:sldIdLst");
                inner.close("p14:section");
            }
            inner.close("p14:sectionLst");
            inner.close("p:ext");
        }
        if let Some(r) = raw {
            inner.raw(r);
        }
        ext_list(&mut w, Some(&inner.s));
    }
    w.close("p:presentation");
    w.finish()
}

pub(super) fn pres_props_xml(p: &Presentation) -> Vec<u8> {
    let s = &p.show;
    let mut w = W::new();
    root_open(&mut w, "p:presentationPr", A::new());
    w.open(
        "p:showPr",
        A::new()
            .t("loop", s.loop_until_esc)
            .a("showNarration", if s.without_narration { 0 } else { 1 })
            .o("showAnimation", s.without_animation.then_some("0"))
            .o("useTimings", (!s.use_timings).then_some("0")),
    );
    match s.show_type.as_str() {
        "browsed" => w.empty("p:browse", A::new()),
        "kiosk" => w.empty("p:kiosk", A::new()),
        _ => w.empty0("p:present"),
    }
    let custom = s.custom_show.as_ref().and_then(|n| p.custom_shows.iter().position(|c| c.name == *n));
    match (custom, s.range) {
        (Some(i), _) => w.empty("p:custShow", A::new().a("id", i)),
        (None, Some((a, b))) => w.empty("p:sldRg", A::new().a("st", a.max(1)).a("end", b.max(a).max(1))),
        _ => w.empty0("p:sldAll"),
    }
    w.open0("p:penClr");
    w.val("a:srgbClr", s.pen_color.hex());
    w.close("p:penClr");
    w.close("p:showPr");
    w.close("p:presentationPr");
    w.finish()
}

pub(super) fn view_props_xml() -> Vec<u8> {
    let mut w = W::new();
    root_open(&mut w, "p:viewPr", A::new());
    w.open0("p:normalViewPr");
    w.empty("p:restoredLeft", A::new().a("sz", 15620));
    w.empty("p:restoredTop", A::new().a("sz", 94660));
    w.close("p:normalViewPr");
    w.empty("p:gridSpacing", A::new().a("cx", 76200).a("cy", 76200));
    w.close("p:viewPr");
    w.finish()
}

pub(super) fn core_xml(p: &Presentation) -> Vec<u8> {
    let pr = &p.props;
    let mut w = W::new();
    w.open(
        "cp:coreProperties",
        A::new()
            .a("xmlns:cp", "http://schemas.openxmlformats.org/package/2006/metadata/core-properties")
            .a("xmlns:dc", "http://purl.org/dc/elements/1.1/")
            .a("xmlns:dcterms", "http://purl.org/dc/terms/")
            .a("xmlns:dcmitype", "http://purl.org/dc/dcmitype/")
            .a("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
    );
    for (tag, v) in [
        ("dc:title", &pr.title),
        ("dc:subject", &pr.subject),
        ("dc:creator", &pr.author),
        ("cp:keywords", &pr.keywords),
        ("dc:description", &pr.comments),
        ("cp:category", &pr.category),
        ("cp:lastModifiedBy", &pr.last_modified_by),
    ] {
        if !v.is_empty() {
            w.elt(tag, v);
        }
    }
    if pr.revision > 0 {
        w.elt("cp:revision", &pr.revision.to_string());
    }
    for (tag, v) in [("dcterms:created", &pr.created), ("dcterms:modified", &pr.modified)] {
        if valid_date(v) {
            w.open(tag, A::new().a("xsi:type", "dcterms:W3CDTF"));
            w.text(v);
            w.close(tag);
        }
    }
    w.close("cp:coreProperties");
    w.finish()
}

pub(super) fn app_xml(p: &Presentation) -> Vec<u8> {
    let mut w = W::new();
    w.open(
        "Properties",
        A::new()
            .a("xmlns", "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties")
            .a("xmlns:vt", "http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"),
    );
    w.elt("Application", "DeckCraft");
    w.elt("Slides", &p.slides.len().to_string());
    w.elt("Notes", &p.slides.iter().filter(|s| !s.notes.is_empty()).count().to_string());
    w.elt("HiddenSlides", &p.slides.iter().filter(|s| s.hidden).count().to_string());
    if !p.props.company.is_empty() {
        w.elt("Company", &p.props.company);
    }
    w.close("Properties");
    w.finish()
}
