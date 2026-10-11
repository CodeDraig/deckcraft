//! PPTX import: package → [`Presentation`].

mod markup;
pub use markup::{keep_xml, resolve_mc};

mod properties;
use properties::{header_footer, read_core, read_show};

mod parts;
pub use parts::color_map;
use parts::{read_master, read_notes, read_slide};

mod chart;
mod comments;
pub(crate) mod dml;
mod shapes;
mod theme;
mod timing;

pub(crate) use chart::{chart_from as chart_from_el, read_chart_xml};

use std::collections::HashMap;
use std::sync::Arc;

use deckcraft_geom::{Size, emu_to_pt};
use deckcraft_model::{CustomShow, LayoutId, Master, MasterId, MediaId, MediaItem, Presentation, Section, SlideId, defaults};

use crate::PptxError;
use crate::opc::{self, Package, Rels};
use crate::xml::{Doc, El};

/// Import state shared by all parts.
pub struct Imp<'a> {
    pub pkg: &'a Package,
    pub media: Vec<MediaItem>,
    pub media_by_part: HashMap<String, MediaId>,
    pub next: u32,
    pub slide_by_part: HashMap<String, SlideId>,
    pub custom_show_names: HashMap<String, String>,
}

impl Imp<'_> {
    pub fn alloc(&mut self) -> u32 {
        self.next = self.next.saturating_add(1);
        self.next
    }
    /// Media item for an internal part, read once.
    pub fn media_for(&mut self, part: &str) -> Option<MediaId> {
        let key = opc::norm(part);
        if let Some(m) = self.media_by_part.get(&key) {
            return Some(*m);
        }
        let data = self.pkg.get(part)?.to_vec();
        let name = opc::file_of(self.pkg.original_name(part).unwrap_or(part)).to_string();
        let content_type = self.pkg.content_type(part).filter(|c| !c.is_empty()).unwrap_or_else(|| crate::mime_for(&name).to_string());
        let id = MediaId(self.alloc());
        self.media.push(MediaItem { id, name, content_type, data: Arc::new(data), link: None });
        self.media_by_part.insert(key, id);
        Some(id)
    }
    /// Media item for an external link.
    pub fn media_link(&mut self, url: &str) -> MediaId {
        if let Some(m) = self.media.iter().find(|m| m.link.as_deref() == Some(url)) {
            return m.id;
        }
        let id = MediaId(self.alloc());
        let name = url.rsplit('/').next().unwrap_or(url).to_string();
        let content_type = crate::mime_for(&name).to_string();
        self.media.push(MediaItem { id, name, content_type, data: Arc::new(vec![]), link: Some(url.to_string()) });
        id
    }
}

/// One part being read: its name, relationships and root namespace declarations.
pub struct Part {
    pub name: String,
    pub rels: Rels,
    pub ns: Vec<(String, String)>,
}

impl Part {
    pub fn new(pkg: &Package, name: &str, doc: &Doc) -> Self {
        Part { name: name.to_string(), rels: pkg.rels(name), ns: doc.ns_decls() }
    }
    /// Serialize an element with the namespace declarations it needs from the part root.
    pub fn keep(&self, el: &El) -> String {
        keep_xml(el, &self.ns)
    }
}

fn load(pkg: &Package, name: &str) -> Option<Doc> {
    let mut d = pkg.xml(name)?;
    resolve_mc(&mut d.root, 0);
    Some(d)
}

fn max_cnvpr(el: &El, m: &mut u32) {
    let mut v = vec![];
    el.find_all("cNvPr", &mut v);
    for c in v {
        if let Some(id) = c.u32("id")
            && id < shapes::MAX_KEPT_ID
        {
            *m = (*m).max(id);
        }
    }
}

pub fn import(bytes: &[u8]) -> Result<Presentation, PptxError> {
    let pkg = Package::open(bytes)?;
    let root_rels = pkg.rels("");
    let pres_path = root_rels.first_of("officeDocument").map(|r| r.target.clone()).unwrap_or_else(|| "ppt/presentation.xml".into());
    let pres_doc = load(&pkg, &pres_path).ok_or_else(|| PptxError::NotPptx(format!("missing or unreadable {pres_path}")))?;
    let pres_el = &pres_doc.root;
    if !pres_el.is("presentation") {
        return Err(PptxError::NotPptx(format!("{pres_path} is not a presentation part")));
    }
    let pres_part = Part::new(&pkg, &pres_path, &pres_doc);

    // Masters, layouts and slides to read.
    let master_paths: Vec<String> = pres_el
        .child("sldMasterIdLst")
        .map(|l| l.children_named("sldMasterId").filter_map(|m| pres_part.rels.target(m.attr("r:id").unwrap_or(""))).map(String::from).collect())
        .unwrap_or_default();
    let master_paths: Vec<String> =
        if master_paths.is_empty() { pres_part.rels.all_of("slideMaster").map(|r| r.target.clone()).collect() } else { master_paths };
    let slide_entries: Vec<(u32, String)> = pres_el
        .child("sldIdLst")
        .map(|l| {
            l.children_named("sldId")
                .take(10_000)
                .filter_map(|s| Some((s.u32("id").unwrap_or(0), pres_part.rels.target(s.attr("r:id").unwrap_or(""))?.to_string())))
                .collect()
        })
        .unwrap_or_default();

    let mut docs: HashMap<String, Doc> = HashMap::new();
    let mut max_id = 0u32;
    let mut all_parts: Vec<String> = master_paths.clone();
    for m in &master_paths {
        all_parts.extend(pkg.rels(m).all_of("slideLayout").map(|r| r.target.clone()));
    }
    all_parts.extend(slide_entries.iter().map(|(_, p)| p.clone()));
    for p in &all_parts {
        let key = opc::norm(p);
        if docs.contains_key(&key) {
            continue;
        }
        if let Some(d) = load(&pkg, p) {
            max_cnvpr(&d.root, &mut max_id);
            docs.insert(key, d);
        }
    }

    let mut imp = Imp {
        pkg: &pkg,
        media: vec![],
        media_by_part: HashMap::new(),
        next: max_id.max(1),
        slide_by_part: HashMap::new(),
        custom_show_names: HashMap::new(),
    };
    // Slide ids first so hyperlinks anywhere can point at slides.
    let mut slide_ids: Vec<(u32, String, SlideId)> = vec![];
    for (file_id, path) in &slide_entries {
        if !docs.contains_key(&opc::norm(path)) {
            log::warn!("pptx: slide {path} is missing or unreadable; skipped");
            continue;
        }
        let id = SlideId(imp.alloc());
        imp.slide_by_part.insert(opc::norm(path), id);
        slide_ids.push((*file_id, path.clone(), id));
    }
    if let Some(l) = pres_el.child("custShowLst") {
        for cs in l.children_named("custShow") {
            if let (Some(id), Some(name)) = (cs.attr("id"), cs.attr("name")) {
                imp.custom_show_names.insert(id.to_string(), name.to_string());
            }
        }
    }

    let mut p = defaults::blank_presentation(defaults::WIDE, Default::default(), false);
    p.masters.clear();
    p.media.clear();
    p.default_text_style = Default::default();

    if let Some(sz) = pres_el.child("sldSz") {
        let w = sz.i64("cx").map(emu_to_pt).unwrap_or(960.0);
        let h = sz.i64("cy").map(emu_to_pt).unwrap_or(540.0);
        p.slide_size = Size::new(sane_len(w, 960.0), sane_len(h, 540.0));
    }
    if let Some(sz) = pres_el.child("notesSz") {
        let w = sz.i64("cx").map(emu_to_pt).unwrap_or(540.0);
        let h = sz.i64("cy").map(emu_to_pt).unwrap_or(720.0);
        p.notes_size = Size::new(sane_len(w, 540.0), sane_len(h, 720.0));
    }
    p.first_slide_number = pres_el.u32("firstSlideNum").unwrap_or(1).min(9999);
    if let Some(dts) = pres_el.child("defaultTextStyle") {
        p.default_text_style = dml::list_style(&mut imp, &pres_part, dts);
    }

    // Masters and layouts.
    let mut layout_by_part: HashMap<String, LayoutId> = HashMap::new();
    for mp in &master_paths {
        let Some(doc) = docs.get(&opc::norm(mp)) else {
            log::warn!("pptx: master {mp} missing; skipped");
            continue;
        };
        let master = read_master(&mut imp, mp, doc, &docs, &mut layout_by_part);
        p.masters.push(Arc::new(master));
    }
    if p.masters.is_empty() {
        log::warn!("pptx: no readable slide master; using a default one");
        let (m, _) = defaults::build_master(p.slide_size, Default::default(), imp.next);
        imp.next = imp.next.saturating_add(200);
        p.masters.push(Arc::new(m));
    }
    let fallback_layout = p.masters.first().and_then(|m| m.layouts.first()).map(|l| l.id).unwrap_or_default();

    let comment_authors = comments::authors(&pkg, &pres_part)?;

    // Slides.
    let mut file_slide_ids: HashMap<u32, SlideId> = HashMap::new();
    for (file_id, path, id) in &slide_ids {
        let Some(doc) = docs.get(&opc::norm(path)) else { continue };
        let part = Part::new(&pkg, path, doc);
        let layout = part.rels.first_of("slideLayout").and_then(|r| layout_by_part.get(&opc::norm(&r.target)).copied()).unwrap_or(fallback_layout);
        let mut shape_ids = shapes::IdCtx::default();
        let mut slide = read_slide(&mut imp, &part, doc, *id, layout, &mut shape_ids);
        // Notes.
        if let Some(n) = part.rels.first_of("notesSlide")
            && let Some(nd) = load(&pkg, &n.target)
        {
            let np = Part::new(&pkg, &n.target, &nd);
            slide.notes = read_notes(&mut imp, &np, &nd.root);
        }
        // Comments.
        for c in part.rels.all_of("comments") {
            if c.external || c.kind_uri != crate::comments::RT_COMMENTS {
                return Err(PptxError::Unsupported("comments must use the modern internal comment relationship".into()));
            }
            slide.comments.extend(comments::read(&pkg, &c.target, &comment_authors, &shape_ids, *file_id)?);
        }
        file_slide_ids.insert(*file_id, *id);
        p.slides.push(Arc::new(slide));
    }

    // Sections.
    if let Some(ext) = pres_el.child("extLst") {
        if let Some(sl) = ext.find("sectionLst") {
            for s in sl.children_named("section").take(10_000) {
                let slides = s
                    .child("sldIdLst")
                    .map(|l| l.children_named("sldId").filter_map(|x| x.u32("id").and_then(|i| file_slide_ids.get(&i).copied())).collect())
                    .unwrap_or_default();
                p.sections.push(Section { name: s.attr("name").unwrap_or("").to_string(), slides });
            }
        }
        let kept: String = ext
            .children_named("ext")
            .filter(|e| e.attr("uri") != Some(crate::SECTION_EXT_URI) && !has_rel_refs(e))
            .map(|e| pres_part.keep(e))
            .collect();
        if !kept.is_empty() {
            p.raw_ext = Some(kept);
        }
    }
    // Custom shows.
    if let Some(l) = pres_el.child("custShowLst") {
        for cs in l.children_named("custShow") {
            let slides = cs
                .child("sldLst")
                .map(|l| {
                    l.children_named("sld")
                        .filter_map(|s| pres_part.rels.target(s.attr("r:id").unwrap_or("")))
                        .filter_map(|t| imp.slide_by_part.get(&opc::norm(t)).copied())
                        .collect()
                })
                .unwrap_or_default();
            p.custom_shows.push(CustomShow { name: cs.attr("name").unwrap_or("").to_string(), slides });
        }
    }
    // Show settings.
    if let Some(r) = pres_part.rels.first_of("presProps")
        && let Some(d) = pkg.xml(&r.target)
        && let Some(sp) = d.root.child("showPr")
    {
        p.show = read_show(sp, &imp.custom_show_names);
    }
    // Notes master.
    if let Some(r) = pres_part.rels.first_of("notesMaster")
        && let Some(d) = load(&pkg, &r.target)
    {
        let part = Part::new(&pkg, &r.target, &d);
        let mut ctx = shapes::IdCtx::default();
        let mut m = Master { id: MasterId(imp.alloc()), name: "Notes Master".into(), ..Default::default() };
        if let Some(t) = part.rels.first_of("theme")
            && let Some(td) = pkg.xml(&t.target)
        {
            let tp = Part::new(&pkg, &t.target, &td);
            m.theme = theme::read_theme(&mut imp, &tp, &td.root);
        }
        if let Some(tree) = d.root.path(&["cSld", "spTree"]) {
            m.shapes = shapes::sp_tree(&mut imp, &part, &mut ctx, tree, 0);
        }
        if let Some(cm) = d.root.child("clrMap") {
            m.color_map = color_map(cm);
        }
        p.notes_master = Some(Arc::new(m));
    }
    // Document properties.
    if let Some(r) = root_rels.first_of("core-properties")
        && let Some(d) = pkg.xml(&r.target)
    {
        read_core(&d.root, &mut p.props);
    }
    if let Some(r) = root_rels.first_of("extended-properties")
        && let Some(d) = pkg.xml(&r.target)
    {
        p.props.company = d.root.child("Company").map(|c| c.text()).unwrap_or_default();
    }

    p.header_footer = header_footer(&p);
    p.media = std::mem::take(&mut imp.media);
    p.next_id = imp.next;
    p.fix_next_id();
    Ok(p)
}

fn sane_len(v: f64, dflt: f64) -> f64 {
    if v.is_finite() && (1.0..=20_000.0).contains(&v) { v } else { dflt }
}

/// Does the element (or a descendant) reference a relationship?
pub fn has_rel_refs(el: &El) -> bool {
    fn rec(e: &El, depth: usize) -> bool {
        if depth > crate::xml::MAX_DEPTH {
            return false;
        }
        let own = e.attrs.iter().any(|(k, v)| {
            !k.starts_with("xmlns")
                && k.contains(':')
                && matches!(k.rsplit(':').next(), Some("id" | "embed" | "link" | "dm" | "lo" | "qs" | "cs" | "pict" | "href"))
                && !v.is_empty()
        });
        own || e.elements().any(|c| rec(c, depth + 1))
    }
    rec(el, 0)
}
