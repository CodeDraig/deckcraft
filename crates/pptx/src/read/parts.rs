//! Read slide, layout and master parts while sharing shape and relationship context.

use super::{Imp, Part, dml, has_rel_refs, shapes, theme, timing};
use crate::opc::{self};
use crate::xml::{Doc, El};
use deckcraft_model::text::TextBody;
use deckcraft_model::{Background, Layout, LayoutId, LayoutType, Master, MasterId, Slide, SlideId};
use std::collections::HashMap;

pub fn color_map(cm: &El) -> Vec<(String, String)> {
    cm.attrs.iter().filter(|(k, _)| !k.starts_with("xmlns")).map(|(k, v)| (k.clone(), v.clone())).collect()
}

pub(super) fn background(imp: &mut Imp, part: &Part, csld: &El) -> Option<Background> {
    let bg = csld.child("bg")?;
    if let Some(pr) = bg.child("bgPr") {
        return Some(Background::Fill { fill: dml::fill_of(imp, part, pr).unwrap_or(deckcraft_model::Fill::None) });
    }
    let r = bg.child("bgRef")?;
    Some(Background::Ref {
        idx: r.u32("idx").unwrap_or(1001),
        color: dml::color(r).unwrap_or(deckcraft_model::ColorRef::scheme(deckcraft_color::SchemeSlot::Bg1)),
    })
}

pub(super) fn read_master(
    imp: &mut Imp,
    path: &str,
    doc: &Doc,
    docs: &HashMap<String, Doc>,
    layout_by_part: &mut HashMap<String, LayoutId>,
) -> Master {
    let pkg = imp.pkg;
    let part = Part::new(pkg, path, doc);
    let root = &doc.root;
    let mut m = Master { id: MasterId(0), preserve: root.bool("preserve").unwrap_or(false), ..Default::default() };
    if let Some(t) = part.rels.first_of("theme")
        && let Some(td) = pkg.xml(&t.target)
    {
        let tp = Part::new(pkg, &t.target, &td);
        m.theme = theme::read_theme(imp, &tp, &td.root);
    }
    m.id = MasterId(imp.alloc());
    if let Some(cm) = root.child("clrMap") {
        m.color_map = color_map(cm);
    }
    if let Some(csld) = root.child("cSld") {
        m.name = csld.attr("name").unwrap_or("").to_string();
        m.background = background(imp, &part, csld);
        if let Some(tree) = csld.child("spTree") {
            let mut ctx = shapes::IdCtx::default();
            m.shapes = shapes::sp_tree(imp, &part, &mut ctx, tree, 0);
        }
    }
    if m.name.is_empty() {
        m.name = m.theme.name.clone();
    }
    if let Some(ts) = root.child("txStyles") {
        if let Some(s) = ts.child("titleStyle") {
            m.title_style = dml::list_style(imp, &part, s);
        }
        if let Some(s) = ts.child("bodyStyle") {
            m.body_style = dml::list_style(imp, &part, s);
        }
        if let Some(s) = ts.child("otherStyle") {
            m.other_style = dml::list_style(imp, &part, s);
        }
    }
    // Layouts in list order, then any other related layouts.
    let mut lpaths: Vec<String> = root
        .child("sldLayoutIdLst")
        .map(|l| l.children_named("sldLayoutId").filter_map(|x| part.rels.target(x.attr("r:id").unwrap_or(""))).map(String::from).collect())
        .unwrap_or_default();
    for r in part.rels.all_of("slideLayout") {
        if !lpaths.iter().any(|x| opc::norm(x) == opc::norm(&r.target)) {
            lpaths.push(r.target.clone());
        }
    }
    for lp in lpaths.iter().take(1000) {
        let key = opc::norm(lp);
        if layout_by_part.contains_key(&key) {
            continue;
        }
        let Some(ld) = docs.get(&key) else { continue };
        let lpart = Part::new(pkg, lp, ld);
        let l = read_layout(imp, &lpart, ld);
        layout_by_part.insert(key, l.id);
        m.layouts.push(l);
    }
    if m.layouts.is_empty() {
        // A master must have a layout for slides to use.
        imp.next = imp.next.saturating_add(1);
        m.layouts.push(Layout {
            id: LayoutId(imp.next),
            name: "Blank".into(),
            kind: LayoutType::Blank,
            show_master_shapes: true,
            ..Default::default()
        });
    }
    if let Some(ext) = root.child("extLst").filter(|e| !has_rel_refs(e)) {
        m.raw_ext = Some(ext.elements().map(|e| part.keep(e)).collect());
    }
    m
}

pub(super) fn read_layout(imp: &mut Imp, part: &Part, doc: &Doc) -> Layout {
    let root = &doc.root;
    let mut l = Layout {
        id: LayoutId(imp.alloc()),
        kind: LayoutType::from_xml(root.attr("type").unwrap_or("cust")),
        show_master_shapes: root.bool("showMasterSp").unwrap_or(true),
        preserve: root.bool("preserve").unwrap_or(false),
        ..Default::default()
    };
    if let Some(csld) = root.child("cSld") {
        l.name = csld.attr("name").unwrap_or("").to_string();
        l.background = background(imp, part, csld);
        if let Some(tree) = csld.child("spTree") {
            let mut ctx = shapes::IdCtx::default();
            l.shapes = shapes::sp_tree(imp, part, &mut ctx, tree, 0);
        }
    }
    if let Some(ext) = root.child("extLst").filter(|e| !has_rel_refs(e)) {
        l.raw_ext = Some(ext.elements().map(|e| part.keep(e)).collect());
    }
    l
}

pub(super) fn read_slide(imp: &mut Imp, part: &Part, doc: &Doc, id: SlideId, layout: LayoutId, ctx: &mut shapes::IdCtx) -> Slide {
    let root = &doc.root;
    let mut s = Slide {
        id,
        layout,
        hidden: root.bool("show") == Some(false),
        show_master_shapes: root.bool("showMasterSp").unwrap_or(true),
        ..Default::default()
    };
    if let Some(csld) = root.child("cSld") {
        s.name = csld.attr("name").unwrap_or("").to_string();
        s.background = background(imp, part, csld);
        if let Some(tree) = csld.child("spTree") {
            s.shapes = shapes::sp_tree(imp, part, ctx, tree, 0);
        }
    }
    if let Some(t) = root.child("transition") {
        s.transition = timing::transition(imp, part, t);
    }
    if let Some(t) = root.child("timing") {
        s.animations = timing::animations(imp, part, ctx, t, &s.shapes);
    }
    if let Some(ext) = root.child("extLst").filter(|e| !has_rel_refs(e)) {
        s.raw_ext = Some(ext.elements().map(|e| part.keep(e)).collect());
    }
    s
}

pub(super) fn read_notes(imp: &mut Imp, part: &Part, root: &El) -> TextBody {
    let Some(tree) = root.path(&["cSld", "spTree"]) else { return TextBody::default() };
    for sp in tree.children_named("sp") {
        let ph = sp.path(&["nvSpPr", "nvPr", "ph"]);
        let is_body = ph.is_some_and(|ph| matches!(ph.attr("type"), Some("body") | None) && ph.attr("type") != Some("sldImg"));
        if is_body && let Some(tx) = sp.child("txBody") {
            let mut t = dml::text_body(imp, part, tx);
            // Notes keep only content: drop body formatting from the notes master placeholder.
            t.list_style = Default::default();
            if t.is_empty() {
                return TextBody::default();
            }
            return t;
        }
    }
    TextBody::default()
}
