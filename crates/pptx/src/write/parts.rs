//! Write slide, layout, master and notes parts with their shape ids.

use super::{Exp, Out, comments, dml, ext_children, ext_list, root_open, shapes, timing};
use crate::xml::{A, W};
use deckcraft_geom::Xfrm;
use deckcraft_model::text::{Paragraph, TextBody};
use deckcraft_model::{Background, Layout, Master, PhType, Placeholder, Shape, ShapeId, Slide, defaults};

pub(super) fn background(w: &mut W, x: &mut Exp, o: &mut Out, bg: &Background) {
    w.open0("p:bg");
    match bg {
        Background::Fill { fill } => {
            w.open0("p:bgPr");
            match fill {
                deckcraft_model::Fill::Background | deckcraft_model::Fill::Group => w.empty0("a:noFill"),
                f => dml::fill(w, x, o, f),
            }
            w.empty0("a:effectLst");
            w.close("p:bgPr");
        }
        Background::Ref { idx, color } => {
            let idx = if (1..=3).contains(idx) || (1001..=1003).contains(idx) { *idx } else { 1001 };
            w.open("p:bgRef", A::new().a("idx", idx));
            dml::color(w, color);
            w.close("p:bgRef");
        }
    }
    w.close("p:bg");
}

pub(super) fn csld(w: &mut W, x: &mut Exp, o: &mut Out, name: &str, bg: Option<&Background>, list: &[Shape]) -> shapes::IdMap {
    w.open("p:cSld", A::new().o("name", (!name.is_empty()).then_some(name)));
    if let Some(bg) = bg {
        background(w, x, o, bg);
    }
    w.open0("p:spTree");
    shapes::tree_header(w);
    let mut ids = shapes::IdMap::build(list);
    {
        let mut cx = shapes::Ctx { ids: &mut ids, written: Default::default() };
        shapes::shapes(w, x, o, &mut cx, list, 0);
    }
    w.close("p:spTree");
    w.close("p:cSld");
    ids
}

pub(super) const CLR_KEYS: [(&str, &str); 12] = [
    ("bg1", "lt1"),
    ("tx1", "dk1"),
    ("bg2", "lt2"),
    ("tx2", "dk2"),
    ("accent1", "accent1"),
    ("accent2", "accent2"),
    ("accent3", "accent3"),
    ("accent4", "accent4"),
    ("accent5", "accent5"),
    ("accent6", "accent6"),
    ("hlink", "hlink"),
    ("folHlink", "folHlink"),
];

pub(super) fn clr_map(w: &mut W, map: &[(String, String)]) {
    let mut a = A::new();
    for (k, d) in CLR_KEYS {
        let v = map
            .iter()
            .find(|(mk, _)| mk == k)
            .map(|(_, v)| v.as_str())
            .filter(|v| deckcraft_color::SchemeSlot::from_xml(v).is_some_and(|s| deckcraft_color::SchemeSlot::THEME.contains(&s)))
            .unwrap_or(d);
        a = a.a(k, v);
    }
    w.empty("p:clrMap", a);
}

pub(super) fn notes_master_shapes(size: deckcraft_geom::Size) -> Vec<Shape> {
    let (w, h) = (size.width, size.height);
    let mk = |id: u32, name: &str, kind: PhType, idx: u32, x: f64, y: f64, ww: f64, hh: f64| Shape {
        id: ShapeId(id),
        name: name.into(),
        xfrm: Some(Xfrm::new(x, y, ww, hh)),
        ph: Some(Placeholder { kind, idx, size: (kind != PhType::SlideImage).then(|| "quarter".to_string()), ..Default::default() }),
        text: (kind != PhType::SlideImage).then(TextBody::default),
        ..Default::default()
    };
    let mut img = mk(4, "Slide Image Placeholder 3", PhType::SlideImage, 2, w * 0.125, h * 0.125, w * 0.75, h * 0.375);
    img.fill = Some(deckcraft_model::Fill::None);
    img.line = Some(deckcraft_model::Line::solid(deckcraft_model::ColorRef::rgb(deckcraft_color::Rgba::BLACK), 1.0));
    let mut body = mk(5, "Notes Placeholder 4", PhType::Body, 3, w * 0.1, h * 0.53, w * 0.8, h * 0.36);
    if let Some(t) = body.text.as_mut() {
        t.paragraphs = vec![Paragraph::new("Click to edit Master text styles")];
    }
    vec![
        mk(2, "Header Placeholder 1", PhType::Header, 0, 0.0, 0.0, w * 0.43, h * 0.05),
        mk(3, "Date Placeholder 2", PhType::Date, 1, w * 0.57, 0.0, w * 0.43, h * 0.05),
        img,
        body,
        mk(6, "Footer Placeholder 5", PhType::Footer, 4, 0.0, h * 0.95, w * 0.43, h * 0.05),
        mk(7, "Slide Number Placeholder 6", PhType::SlideNum, 5, w * 0.57, h * 0.95, w * 0.43, h * 0.05),
    ]
}

pub(super) fn notes_style() -> deckcraft_model::text::ListStyle {
    let mut ls = deckcraft_model::text::ListStyle::default();
    for i in 0..9u8 {
        ls.set(
            i,
            deckcraft_model::text::LevelStyle {
                para: deckcraft_model::text::ParaProps {
                    margin_left: Some(36.0 * i as f64),
                    align: Some(deckcraft_model::text::Align::Left),
                    ..Default::default()
                },
                run: deckcraft_model::text::RunProps {
                    size: Some(12.0),
                    font: Some("+mn-lt".into()),
                    fill: Some(deckcraft_model::Fill::solid(deckcraft_model::ColorRef::scheme(deckcraft_color::SchemeSlot::Tx1))),
                    ..Default::default()
                },
            },
        );
    }
    ls
}

pub(super) fn layout_xml(x: &mut Exp, o: &mut Out, l: &Layout) -> Vec<u8> {
    let mut w = W::new();
    root_open(
        &mut w,
        "p:sldLayout",
        A::new().o("showMasterSp", (!l.show_master_shapes).then_some("0")).a("type", l.kind.xml()).t("preserve", l.preserve),
    );
    csld(&mut w, x, o, &l.name, l.background.as_ref(), &l.shapes);
    w.open0("p:clrMapOvr");
    w.empty0("a:masterClrMapping");
    w.close("p:clrMapOvr");
    ext_list(&mut w, ext_children(l.raw_ext.as_deref()).as_deref());
    w.close("p:sldLayout");
    w.finish()
}

pub(super) fn master_xml(x: &mut Exp, o: &mut Out, m: &Master, layouts: &[(u64, String)]) -> Vec<u8> {
    let mut w = W::new();
    root_open(&mut w, "p:sldMaster", A::new().t("preserve", m.preserve));
    csld(&mut w, x, o, &m.name, m.background.as_ref(), &m.shapes);
    clr_map(&mut w, &m.color_map);
    w.open0("p:sldLayoutIdLst");
    for (id, rid) in layouts {
        w.empty("p:sldLayoutId", A::new().a("id", id).a("r:id", rid));
    }
    w.close("p:sldLayoutIdLst");
    w.open0("p:txStyles");
    let (dt, db, dother) = defaults::master_styles();
    let pick = |s: &deckcraft_model::text::ListStyle, d: deckcraft_model::text::ListStyle| if s.is_empty() { d } else { s.clone() };
    dml::list_style(&mut w, x, o, "p:titleStyle", &pick(&m.title_style, dt));
    dml::list_style(&mut w, x, o, "p:bodyStyle", &pick(&m.body_style, db));
    dml::list_style(&mut w, x, o, "p:otherStyle", &pick(&m.other_style, dother));
    w.close("p:txStyles");
    ext_list(&mut w, ext_children(m.raw_ext.as_deref()).as_deref());
    w.close("p:sldMaster");
    w.finish()
}

pub(super) fn slide_xml(x: &mut Exp, o: &mut Out, s: &Slide, comment_rid: Option<&str>) -> Vec<u8> {
    let mut w = W::new();
    root_open(&mut w, "p:sld", A::new().o("showMasterSp", (!s.show_master_shapes).then_some("0")).o("show", s.hidden.then_some("0")));
    let ids = csld(&mut w, x, o, &s.name, s.background.as_ref(), &s.shapes);
    w.open0("p:clrMapOvr");
    w.empty0("a:masterClrMapping");
    w.close("p:clrMapOvr");
    if let Some(t) = &s.transition {
        timing::transition(&mut w, x, o, t);
    }
    if !s.animations.is_empty() {
        timing::timing(&mut w, x, &s.animations, &s.shapes, &ids);
    }
    let mut extensions = W::frag();
    if let Some(rid) = comment_rid {
        comments::relationship(&mut extensions, rid);
    }
    if let Some(raw) = &s.raw_ext {
        extensions.raw(raw);
    }
    ext_list(&mut w, Some(&String::from_utf8_lossy(&extensions.finish())));
    w.close("p:sld");
    w.finish()
}

pub(super) fn notes_xml(x: &mut Exp, o: &mut Out, notes: &TextBody) -> Vec<u8> {
    let mut w = W::new();
    root_open(&mut w, "p:notes", A::new());
    let img = Shape {
        id: ShapeId(2),
        name: "Slide Image Placeholder 1".into(),
        ph: Some(Placeholder { kind: PhType::SlideImage, ..Default::default() }),
        ..Default::default()
    };
    let mut tb = notes.clone();
    tb.body = Default::default();
    let body = Shape {
        id: ShapeId(3),
        name: "Notes Placeholder 2".into(),
        ph: Some(Placeholder { kind: PhType::Body, idx: 1, ..Default::default() }),
        text: Some(tb),
        ..Default::default()
    };
    csld(&mut w, x, o, "", None, &[img, body]);
    w.open0("p:clrMapOvr");
    w.empty0("a:masterClrMapping");
    w.close("p:clrMapOvr");
    w.close("p:notes");
    w.finish()
}
