//! PPTX export: [`Presentation`] → package.

mod properties;
use properties::{app_xml, core_xml, pres_props_xml, presentation_xml, view_props_xml};

mod parts;
use parts::{clr_map, csld, layout_xml, master_xml, notes_master_shapes, notes_style, notes_xml, slide_xml};

mod chart;
mod comments;
pub(crate) mod dml;
mod shapes;
mod theme;
mod timing;

use std::collections::HashMap;
use std::sync::Arc;

use deckcraft_model::{Background, Layout, LayoutId, MediaId, Presentation, SlideId, defaults};

use crate::PptxError;
use crate::opc::{self, NS_A, NS_P, NS_R, PackageOut, RelsOut};
use crate::xml::{A, W};

const CT_PRES: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
const CT_SLIDE: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const CT_LAYOUT: &str = "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml";
const CT_MASTER: &str = "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml";
const CT_NOTES_MASTER: &str = "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml";
const CT_NOTES: &str = "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";
const CT_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
const CT_PRES_PROPS: &str = "application/vnd.openxmlformats-officedocument.presentationml.presProps+xml";
const CT_VIEW_PROPS: &str = "application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml";
const CT_TABLE_STYLES: &str = "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml";
const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";

const CT_CORE: &str = "application/vnd.openxmlformats-package.core-properties+xml";
const CT_APP: &str = "application/vnd.openxmlformats-officedocument.extended-properties+xml";

/// Export state.
pub struct Exp<'a> {
    pub p: &'a Presentation,
    pub pkg: PackageOut,
    media_parts: HashMap<MediaId, String>,
    n_image: u32,
    n_media: u32,
    n_chart: u32,
    pub slide_parts: HashMap<SlideId, String>,
    pub table_styles: Vec<String>,
    placeholder: Option<String>,
}

/// A part being written: its name and relationships.
pub struct Out {
    pub name: String,
    pub rels: RelsOut,
}

impl Out {
    fn new(name: &str) -> Self {
        Out { name: name.to_string(), rels: RelsOut::default() }
    }
}

fn ext_for(content_type: &str, name: &str, data: &[u8]) -> String {
    let by_ct = match content_type {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpeg",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        "image/svg+xml" => "svg",
        "image/x-emf" | "image/emf" => "emf",
        "image/x-wmf" | "image/wmf" => "wmf",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        "video/quicktime" => "mov",
        "video/x-ms-wmv" => "wmv",
        "video/x-msvideo" => "avi",
        "video/webm" => "webm",
        "audio/mpeg" => "mp3",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/mp4" | "audio/x-m4a" => "m4a",
        "audio/x-ms-wma" => "wma",
        "audio/ogg" => "ogg",
        "audio/flac" => "flac",
        _ => "",
    };
    if !by_ct.is_empty() {
        return by_ct.to_string();
    }
    if data.starts_with(b"\x89PNG") {
        return "png".into();
    }
    if data.starts_with(&[0xFF, 0xD8]) {
        return "jpeg".into();
    }
    if data.starts_with(b"GIF8") {
        return "gif".into();
    }
    let e = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    if !e.is_empty() && e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()) { e } else { "bin".into() }
}

impl Exp<'_> {
    fn media_part(&mut self, id: MediaId) -> Option<String> {
        if let Some(p) = self.media_parts.get(&id) {
            return Some(p.clone());
        }
        let m = self.p.media(id)?;
        if m.data.is_empty() {
            return None;
        }
        let ext = ext_for(&m.content_type, &m.name, &m.data);
        let ct = if m.content_type.is_empty() || !m.content_type.contains('/') {
            crate::mime_for(&format!("x.{ext}")).to_string()
        } else {
            m.content_type.clone()
        };
        let name = if ct.starts_with("image/") {
            self.n_image += 1;
            format!("ppt/media/image{}.{ext}", self.n_image)
        } else {
            self.n_media += 1;
            format!("ppt/media/media{}.{ext}", self.n_media)
        };
        self.pkg.binary(&name, &ct, m.data.as_ref().clone());
        self.media_parts.insert(id, name.clone());
        Some(name)
    }
    /// A relationship from `o` to a media item (embedded part, or external link).
    pub fn media_rel(&mut self, o: &mut Out, id: MediaId, kind: &str) -> Option<String> {
        let m = self.p.media(id)?;
        if m.data.is_empty() {
            let link = m.link.clone()?;
            return Some(o.rels.add(kind, &link, true));
        }
        let part = self.media_part(id)?;
        Some(o.rels.add(kind, &opc::relative(&o.name, &part), false))
    }
    pub fn media_rel_new(&mut self, o: &mut Out, id: MediaId, kind: &str) -> Option<String> {
        let m = self.p.media(id)?;
        if m.data.is_empty() {
            return None;
        }
        let part = self.media_part(id)?;
        Some(o.rels.add(kind, &opc::relative(&o.name, &part), false))
    }
    /// A tiny transparent PNG for pictures whose image is missing.
    pub fn placeholder_rel(&mut self, o: &mut Out) -> String {
        let part = match &self.placeholder {
            Some(p) => p.clone(),
            None => {
                self.n_image += 1;
                let name = format!("ppt/media/image{}.png", self.n_image);
                self.pkg.binary(&name, "image/png", crate::png::transparent_1x1());
                self.placeholder = Some(name.clone());
                name
            }
        };
        o.rels.add("image", &opc::relative(&o.name, &part), false)
    }
    pub fn chart_rel(&mut self, o: &mut Out, c: &deckcraft_model::Chart) -> String {
        self.n_chart += 1;
        let name = format!("ppt/charts/chart{}.xml", self.n_chart);
        self.pkg.xml_part(&name, CT_CHART, chart::chart_xml(c));
        o.rels.add_new("chart", &opc::relative(&o.name, &name), false)
    }
}

fn root_open(w: &mut W, tag: &str, extra: A) {
    let mut a = A::new().a("xmlns:a", NS_A).a("xmlns:r", NS_R).a("xmlns:p", NS_P);
    a.v.extend(extra.v);
    w.open(tag, a);
}

fn ext_list(w: &mut W, raw: Option<&str>) {
    let Some(raw) = raw.filter(|r| !r.trim().is_empty()) else { return };
    // Only keep well-formed extension elements.
    let Ok(d) = crate::xml::parse(format!("<x>{raw}</x>").as_bytes()) else { return };
    let exts: Vec<String> = d.root.elements().filter(|e| e.is("ext") && e.has_attr("uri")).map(|e| e.to_xml()).collect();
    if exts.is_empty() {
        return;
    }
    w.open0("p:extLst");
    for e in exts {
        w.raw(&e);
    }
    w.close("p:extLst");
}

/// Strip an `<p:extLst>` wrapper (masters/layouts keep the whole element).
fn ext_children(raw: Option<&str>) -> Option<String> {
    let raw = raw?;
    let d = crate::xml::parse(raw.as_bytes()).ok()?;
    if d.root.is("extLst") {
        let ns = d.ns_decls();
        return Some(d.root.elements().map(|e| crate::read::keep_xml(e, &ns)).collect());
    }
    Some(raw.to_string())
}

/// A copy of the presentation with every slide pointing at a layout that exists.
fn sanitized(p: &Presentation) -> std::borrow::Cow<'_, Presentation> {
    let mut need = p.masters.is_empty() || p.masters.iter().any(|m| m.layouts.is_empty());
    need |= p.slides.iter().any(|s| p.layout(s.layout).is_none());
    if !need {
        return std::borrow::Cow::Borrowed(p);
    }
    let mut q = p.clone();
    if q.masters.is_empty() {
        let (m, next) = defaults::build_master(q.slide_size, Default::default(), q.next_id.max(1));
        q.next_id = next;
        q.masters.push(Arc::new(m));
    }
    for m in q.masters.iter_mut() {
        if m.layouts.is_empty() {
            let id = LayoutId(q.next_id.saturating_add(1));
            q.next_id = id.0;
            Arc::make_mut(m).layouts.push(Layout {
                id,
                name: "Blank".into(),
                kind: deckcraft_model::LayoutType::Blank,
                show_master_shapes: true,
                ..Default::default()
            });
        }
    }
    let first = q.masters.first().and_then(|m| m.layouts.first()).map(|l| l.id).unwrap_or_default();
    let ok: Vec<LayoutId> = q.masters.iter().flat_map(|m| m.layouts.iter().map(|l| l.id)).collect();
    for s in q.slides.iter_mut() {
        if !ok.contains(&s.layout) {
            Arc::make_mut(s).layout = first;
        }
    }
    std::borrow::Cow::Owned(q)
}

pub fn export(p: &Presentation) -> Result<Vec<u8>, PptxError> {
    let p = sanitized(p);
    let p: &Presentation = &p;
    let mut x = Exp {
        p,
        pkg: PackageOut::default(),
        media_parts: HashMap::new(),
        n_image: 0,
        n_media: 0,
        n_chart: 0,
        slide_parts: HashMap::new(),
        table_styles: vec![],
        placeholder: None,
    };
    for (i, s) in p.slides.iter().enumerate() {
        x.slide_parts.insert(s.id, format!("ppt/slides/slide{}.xml", i + 1));
    }
    let mut pres = Out::new("ppt/presentation.xml");

    // Masters, themes and layouts.
    let mut master_rids = vec![];
    let mut layout_parts: HashMap<LayoutId, String> = HashMap::new();
    let mut n_layout = 0u32;
    let mut big_id: u64 = 2_147_483_648;
    for (mi, m) in p.masters.iter().enumerate() {
        let mname = format!("ppt/slideMasters/slideMaster{}.xml", mi + 1);
        let tname = format!("ppt/theme/theme{}.xml", mi + 1);
        let mut mo = Out::new(&mname);
        let mut lrids = vec![];
        for l in &m.layouts {
            n_layout += 1;
            let lname = format!("ppt/slideLayouts/slideLayout{n_layout}.xml");
            layout_parts.insert(l.id, lname.clone());
            let rid = mo.rels.add("slideLayout", &opc::relative(&mname, &lname), false);
            lrids.push(rid);
            let mut lo = Out::new(&lname);
            lo.rels.add("slideMaster", &opc::relative(&lname, &mname), false);
            let xml = layout_xml(&mut x, &mut lo, l);
            x.pkg.xml_part(&lname, CT_LAYOUT, xml);
            x.pkg.rels(&lname, &lo.rels);
        }
        mo.rels.add("theme", &opc::relative(&mname, &tname), false);
        let mut to = Out::new(&tname);
        let txml = theme::theme_xml(&mut x, &mut to, &m.theme);
        x.pkg.xml_part(&tname, CT_THEME, txml);
        x.pkg.rels(&tname, &to.rels);
        let master_id = big_id;
        big_id += 1;
        let mut layout_ids = vec![];
        for rid in lrids {
            layout_ids.push((big_id, rid));
            big_id += 1;
        }
        let xml = master_xml(&mut x, &mut mo, m, &layout_ids);
        x.pkg.xml_part(&mname, CT_MASTER, xml);
        x.pkg.rels(&mname, &mo.rels);
        master_rids.push((master_id, pres.rels.add("slideMaster", &opc::relative("ppt/presentation.xml", &mname), false)));
    }

    // Notes master when any slide has notes.
    let has_notes = p.slides.iter().any(|s| !s.notes.is_empty());
    let notes_master = "ppt/notesMasters/notesMaster1.xml";
    let mut notes_master_rid = None;
    if has_notes {
        let tname = format!("ppt/theme/theme{}.xml", p.masters.len() + 1);
        let mut no = Out::new(notes_master);
        no.rels.add("theme", &opc::relative(notes_master, &tname), false);
        let theme = p.notes_master.as_ref().map(|m| m.theme.clone()).or_else(|| p.masters.first().map(|m| m.theme.clone())).unwrap_or_default();
        let mut to = Out::new(&tname);
        let txml = theme::theme_xml(&mut x, &mut to, &theme);
        x.pkg.xml_part(&tname, CT_THEME, txml);
        x.pkg.rels(&tname, &to.rels);
        let mut w = W::new();
        root_open(&mut w, "p:notesMaster", A::new());
        let bg = Background::Ref { idx: 1001, color: deckcraft_model::ColorRef::scheme(deckcraft_color::SchemeSlot::Bg1) };
        csld(&mut w, &mut x, &mut no, "", Some(&bg), &notes_master_shapes(p.notes_size));
        clr_map(&mut w, &[]);
        dml::list_style(&mut w, &mut x, &mut no, "p:notesStyle", &notes_style());
        w.close("p:notesMaster");
        x.pkg.xml_part(notes_master, CT_NOTES_MASTER, w.finish());
        x.pkg.rels(notes_master, &no.rels);
        notes_master_rid = Some(pres.rels.add("notesMaster", &opc::relative("ppt/presentation.xml", notes_master), false));
    }

    // Comment authors.
    let mut authors = comments::Authors::default();
    let mut n_comments = 0u32;

    // Slides.
    let mut slide_rids = vec![];
    for (i, s) in p.slides.iter().enumerate() {
        let sname = format!("ppt/slides/slide{}.xml", i + 1);
        let mut so = Out::new(&sname);
        if let Some(lp) = layout_parts.get(&s.layout) {
            so.rels.add("slideLayout", &opc::relative(&sname, lp), false);
        }
        if !s.notes.is_empty() {
            let nname = format!("ppt/notesSlides/notesSlide{}.xml", i + 1);
            so.rels.add("notesSlide", &opc::relative(&sname, &nname), false);
            let mut no = Out::new(&nname);
            no.rels.add("notesMaster", &opc::relative(&nname, notes_master), false);
            no.rels.add("slide", &opc::relative(&nname, &sname), false);
            let xml = notes_xml(&mut x, &mut no, &s.notes);
            x.pkg.xml_part(&nname, CT_NOTES, xml);
            x.pkg.rels(&nname, &no.rels);
        }
        let mut comment_rid = None;
        if !s.comments.is_empty() {
            n_comments += 1;
            let cname = format!("ppt/comments/comment{n_comments}.xml");
            comment_rid = Some(so.rels.add(crate::comments::RT_COMMENTS, &opc::relative(&sname, &cname), false));
            let slide_id = u32::try_from(i).ok().and_then(|i| i.checked_add(256)).ok_or_else(|| PptxError::Write("too many slides".into()))?;
            let xml = comments::slide_xml(s, slide_id, &shapes::IdMap::build(&s.shapes), &mut authors)?;
            x.pkg.xml_part(&cname, crate::comments::CT_COMMENTS, xml);
        }
        let xml = slide_xml(&mut x, &mut so, s, comment_rid.as_deref());
        x.pkg.xml_part(&sname, CT_SLIDE, xml);
        x.pkg.rels(&sname, &so.rels);
        slide_rids.push(pres.rels.add("slide", &opc::relative("ppt/presentation.xml", &sname), false));
    }
    if !authors.is_empty() {
        x.pkg.xml_part("ppt/authors.xml", crate::comments::CT_AUTHORS, authors.xml());
        pres.rels.add(crate::comments::RT_AUTHORS, "authors.xml", false);
    }

    // Presentation-level parts.
    let theme1 = "theme/theme1.xml";
    pres.rels.add("theme", theme1, false);
    pres.rels.add("presProps", "presProps.xml", false);
    pres.rels.add("viewProps", "viewProps.xml", false);
    pres.rels.add("tableStyles", "tableStyles.xml", false);
    x.pkg.xml_part("ppt/presProps.xml", CT_PRES_PROPS, pres_props_xml(p));
    x.pkg.xml_part("ppt/viewProps.xml", CT_VIEW_PROPS, view_props_xml());
    let ts = crate::tables::table_styles_xml(&x.table_styles);
    x.pkg.xml_part("ppt/tableStyles.xml", CT_TABLE_STYLES, ts);
    let pxml = presentation_xml(&mut x, &mut pres, &master_rids, notes_master_rid.as_deref(), &slide_rids);
    x.pkg.xml_part("ppt/presentation.xml", CT_PRES, pxml);
    x.pkg.rels("ppt/presentation.xml", &pres.rels);

    // Document properties and package relationships.
    x.pkg.xml_part("docProps/core.xml", CT_CORE, core_xml(p));
    x.pkg.xml_part("docProps/app.xml", CT_APP, app_xml(p));
    let mut root = RelsOut::default();
    root.add("officeDocument", "ppt/presentation.xml", false);
    root.add(opc::RT_CORE, "docProps/core.xml", false);
    root.add("extended-properties", "docProps/app.xml", false);
    x.pkg.rels("", &root);
    x.pkg.finish()
}
