//! Write DrawingML text properties, paragraphs and hyperlinks.

use super::super::{Exp, Out};
use super::{color, effects, emu, fill, line, pct, solid};
use crate::xml::{A, W};
use deckcraft_model::style::{Effects, Fill};
use deckcraft_model::text::{
    Action, AutoFit, BodyProps, Bullet, Caps, Hyperlink, ListStyle, ParaProps, Paragraph, RunKind, RunProps, Spacing, Strike, TextBody,
};

pub fn hyperlink(w: &mut W, x: &mut Exp, o: &mut Out, tag: &str, h: &Hyperlink) {
    let (rid, action) = match &h.action {
        Action::Url { url } => {
            if url.is_empty() {
                return;
            }
            (o.rels.add("hyperlink", url, true), None)
        }
        Action::Slide { slide } => match x.slide_parts.get(slide) {
            Some(target) => {
                let rel = crate::opc::relative(&o.name, target);
                (o.rels.add("slide", &rel, false), Some("ppaction://hlinksldjump".to_string()))
            }
            None => return,
        },
        Action::NextSlide => (String::new(), Some("ppaction://hlinkshowjump?jump=nextslide".into())),
        Action::PreviousSlide => (String::new(), Some("ppaction://hlinkshowjump?jump=previousslide".into())),
        Action::FirstSlide => (String::new(), Some("ppaction://hlinkshowjump?jump=firstslide".into())),
        Action::LastSlide => (String::new(), Some("ppaction://hlinkshowjump?jump=lastslide".into())),
        Action::EndShow => (String::new(), Some("ppaction://hlinkshowjump?jump=endshow".into())),
        Action::LastViewed => (String::new(), Some("ppaction://hlinkshowjump?jump=lastslideviewed".into())),
        Action::CustomShow { name } => {
            let id = x.p.custom_shows.iter().position(|c| c.name == *name).unwrap_or(0);
            (String::new(), Some(format!("ppaction://customshow?id={id}&return=true")))
        }
        Action::Program { path } => {
            if path.is_empty() {
                return;
            }
            (o.rels.add("hyperlink", path, true), Some("ppaction://program".into()))
        }
        Action::PlayMedia => (String::new(), Some("ppaction://media".into())),
    };
    w.empty(
        tag,
        A::new()
            .a("r:id", rid)
            .o("action", action)
            .o("tooltip", (!h.tooltip.is_empty()).then_some(h.tooltip.as_str()))
            .t("highlightClick", h.highlight_click),
    );
}

pub fn body_pr(w: &mut W, b: &BodyProps) {
    let a = A::new()
        .o("rot", b.rot.map(|r| (r * 60_000.0).round().clamp(-21_600_000.0, 21_600_000.0) as i64))
        .o("vert", b.vert.map(|v| v.xml()))
        .o("wrap", b.wrap.map(|v| if v { "square" } else { "none" }))
        .o("lIns", b.inset_l.map(emu))
        .o("tIns", b.inset_t.map(emu))
        .o("rIns", b.inset_r.map(emu))
        .o("bIns", b.inset_b.map(emu))
        .o("numCol", b.columns.map(|c| c.clamp(1, 16)))
        .o("spcCol", b.col_spacing.map(|v| emu(v).max(0)))
        .b("rtlCol", b.rtl_col)
        .o("anchor", b.anchor.map(|a| a.xml()))
        .b("anchorCtr", b.anchor_ctr)
        .b("upright", b.upright);
    let warp = b.warp.as_deref().filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric()));
    if warp.is_none() && b.autofit.is_none() {
        w.empty("a:bodyPr", a);
        return;
    }
    w.open("a:bodyPr", a);
    if let Some(p) = warp {
        w.open("a:prstTxWarp", A::new().a("prst", p));
        w.empty0("a:avLst");
        w.close("a:prstTxWarp");
    }
    match b.autofit {
        Some(AutoFit::None) => w.empty0("a:noAutofit"),
        Some(AutoFit::Shape) => w.empty0("a:spAutoFit"),
        Some(AutoFit::Shrink { font_scale, line_reduction }) => {
            let fs = pct(font_scale.clamp(0.01, 1.0));
            let lr = pct(line_reduction.clamp(0.0, 1.0));
            w.empty("a:normAutofit", A::new().o("fontScale", (fs != 100_000).then_some(fs)).o("lnSpcReduction", (lr != 0).then_some(lr)));
        }
        None => {}
    }
    w.close("a:bodyPr");
}

pub(super) fn spacing(w: &mut W, tag: &str, s: &Spacing) {
    w.open0(tag);
    match s {
        Spacing::Pct(v) => w.val("a:spcPct", pct(v.clamp(0.0, 132.0))),
        Spacing::Pts(v) => w.val("a:spcPts", (v.clamp(0.0, 1584.0) * 100.0).round() as i64),
    }
    w.close(tag);
}

pub fn is_default_ppr(p: &ParaProps) -> bool {
    *p == ParaProps::default()
}

/// `a:pPr` / `a:lvlNpPr`.
pub fn ppr(w: &mut W, x: &mut Exp, o: &mut Out, tag: &str, level: Option<u8>, p: &ParaProps, def: Option<&RunProps>) {
    let a = A::new()
        .o("marL", p.margin_left.map(|v| emu(v.clamp(0.0, 4032.0))))
        .o("marR", p.margin_right.map(|v| emu(v.clamp(0.0, 4032.0))))
        .o("lvl", level.filter(|l| *l > 0).map(|l| l.min(8)))
        .o("indent", p.indent.map(|v| emu(v.clamp(-4032.0, 4032.0))))
        .o("algn", p.align.map(|a| a.xml()))
        .o("defTabSz", p.default_tab.map(|v| emu(v).max(0)))
        .b("rtl", p.rtl)
        .b("eaLnBrk", p.east_asian_line_break)
        .o("fontAlgn", p.font_align.as_deref().filter(|f| matches!(*f, "auto" | "t" | "ctr" | "base" | "b")));
    let has_kids = p.line_spacing.is_some()
        || p.space_before.is_some()
        || p.space_after.is_some()
        || p.bullet.is_some()
        || p.bullet_font.is_some()
        || p.bullet_color.is_some()
        || p.bullet_size.is_some()
        || p.tabs.is_some()
        || def.is_some();
    if !has_kids {
        w.empty(tag, a);
        return;
    }
    w.open(tag, a);
    if let Some(s) = &p.line_spacing {
        spacing(w, "a:lnSpc", s);
    }
    if let Some(s) = &p.space_before {
        spacing(w, "a:spcBef", s);
    }
    if let Some(s) = &p.space_after {
        spacing(w, "a:spcAft", s);
    }
    if let Some(c) = &p.bullet_color {
        w.open0("a:buClr");
        color(w, c);
        w.close("a:buClr");
    }
    if let Some(s) = p.bullet_size {
        w.val("a:buSzPct", pct(s.clamp(0.25, 4.0)));
    }
    if let Some(f) = &p.bullet_font {
        w.empty("a:buFont", A::new().a("typeface", f));
    }
    match &p.bullet {
        Some(Bullet::None) => w.empty0("a:buNone"),
        Some(Bullet::Char { char }) => w.empty("a:buChar", A::new().a("char", if char.is_empty() { "•" } else { char })),
        Some(Bullet::AutoNum { scheme, start_at }) => {
            let s = if scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric()) { "arabicPeriod" } else { scheme };
            w.empty("a:buAutoNum", A::new().a("type", s).o("startAt", (*start_at != 1).then_some((*start_at).clamp(1, 32767))));
        }
        Some(Bullet::Picture { media }) => match x.media_rel(o, *media, "image") {
            Some(rid) => {
                w.open0("a:buBlip");
                w.empty("a:blip", A::new().a("r:embed", rid));
                w.close("a:buBlip");
            }
            None => w.empty("a:buChar", A::new().a("char", "•")),
        },
        None => {}
    }
    if let Some(t) = &p.tabs {
        w.open0("a:tabLst");
        for tab in t.iter().take(32) {
            let al = match tab.align.as_str() {
                "ctr" | "r" | "dec" => tab.align.as_str(),
                _ => "l",
            };
            w.empty("a:tab", A::new().a("pos", emu(tab.pos)).a("algn", al));
        }
        w.close("a:tabLst");
    }
    if let Some(d) = def {
        rpr(w, x, o, "a:defRPr", d);
    }
    w.close(tag);
}

pub(super) fn typeface(w: &mut W, tag: &str, f: &Option<String>) {
    if let Some(f) = f {
        w.empty(tag, A::new().a("typeface", f));
    }
}

/// `a:rPr`, `a:defRPr`, `a:endParaRPr`.
pub fn rpr(w: &mut W, x: &mut Exp, o: &mut Out, tag: &str, r: &RunProps) {
    let a = A::new()
        .o("lang", r.lang.as_deref().filter(|l| !l.is_empty()))
        .o("sz", r.size.map(|s| (s.clamp(1.0, 4000.0) * 100.0).round() as i64))
        .b("b", r.bold)
        .b("i", r.italic)
        .o("u", r.underline.as_deref().filter(|u| !u.is_empty() && u.chars().all(|c| c.is_ascii_alphanumeric())))
        .o(
            "strike",
            r.strike.map(|s| match s {
                Strike::None => "noStrike",
                Strike::Single => "sngStrike",
                Strike::Double => "dblStrike",
            }),
        )
        .o("kern", r.kern.map(|k| (k.clamp(0.0, 4000.0) * 100.0).round() as i64))
        .o(
            "cap",
            r.caps.map(|c| match c {
                Caps::None => "none",
                Caps::Small => "small",
                Caps::All => "all",
            }),
        )
        .o("spc", r.spacing.map(|s| (s.clamp(-4000.0, 4000.0) * 100.0).round() as i64))
        .o("baseline", r.baseline.map(|b| pct(b.clamp(-10.0, 10.0))))
        .b("noProof", r.no_proof);
    let has_kids = r.outline.is_some()
        || r.fill.is_some()
        || r.shadow.is_some()
        || r.glow.is_some()
        || r.highlight.is_some()
        || r.underline_color.is_some()
        || r.font.is_some()
        || r.font_ea.is_some()
        || r.font_cs.is_some()
        || r.font_sym.is_some()
        || r.link.is_some();
    if !has_kids {
        w.empty(tag, a);
        return;
    }
    w.open(tag, a);
    if let Some(l) = &r.outline {
        line(w, x, o, "a:ln", l);
    }
    if let Some(f) = &r.fill {
        match f {
            Fill::Background => {}
            f => fill(w, x, o, f),
        }
    }
    if r.shadow.is_some() || r.glow.is_some() {
        effects(w, &Effects { outer_shadow: r.shadow.clone(), glow: r.glow.clone(), ..Default::default() });
    }
    if let Some(h) = &r.highlight {
        w.open0("a:highlight");
        color(w, h);
        w.close("a:highlight");
    }
    if let Some(c) = &r.underline_color {
        w.open0("a:uFill");
        solid(w, c);
        w.close("a:uFill");
    }
    typeface(w, "a:latin", &r.font);
    typeface(w, "a:ea", &r.font_ea);
    typeface(w, "a:cs", &r.font_cs);
    typeface(w, "a:sym", &r.font_sym);
    if let Some(h) = &r.link {
        hyperlink(w, x, o, "a:hlinkClick", h);
    }
    w.close(tag);
}

/// `a:lstStyle`-like element (`a:lstStyle`, `p:titleStyle`, `p:defaultTextStyle`…).
pub fn list_style(w: &mut W, x: &mut Exp, o: &mut Out, tag: &str, ls: &ListStyle) {
    if ls.is_empty() {
        w.empty0(tag);
        return;
    }
    w.open0(tag);
    for (i, l) in ls.levels.iter().enumerate().take(9) {
        if let Some(l) = l {
            let t = format!("a:lvl{}pPr", i + 1);
            let def = (l.run != RunProps::default()).then_some(&l.run);
            ppr(w, x, o, &t, None, &l.para, def);
        }
    }
    w.close(tag);
}

/// Write `<tag>` text body (`p:txBody`, `a:txBody`). `math` selects the OMML form of equations.
pub fn text_body(w: &mut W, x: &mut Exp, o: &mut Out, tag: &str, tb: &TextBody, math: bool) {
    w.open0(tag);
    body_pr(w, &tb.body);
    list_style(w, x, o, "a:lstStyle", &tb.list_style);
    if tb.paragraphs.is_empty() {
        w.empty0("a:p");
    }
    for (pi, p) in tb.paragraphs.iter().enumerate() {
        paragraph(w, x, o, p, math, pi);
    }
    w.close(tag);
}

pub(super) fn run_text(w: &mut W, x: &mut Exp, o: &mut Out, text: &str, props: &RunProps) {
    for (i, piece) in text.split(['\n', '\u{b}']).enumerate() {
        if i > 0 {
            w.open0("a:br");
            rpr(w, x, o, "a:rPr", props);
            w.close("a:br");
        }
        if piece.is_empty() && i > 0 {
            continue;
        }
        w.open0("a:r");
        rpr(w, x, o, "a:rPr", props);
        w.elt("a:t", piece);
        w.close("a:r");
    }
}

pub fn paragraph(w: &mut W, x: &mut Exp, o: &mut Out, p: &Paragraph, math: bool, index: usize) {
    w.open0("a:p");
    if p.level > 0 || !is_default_ppr(&p.props) {
        ppr(w, x, o, "a:pPr", Some(p.level), &p.props, None);
    }
    for (ri, r) in p.runs.iter().enumerate() {
        match &r.kind {
            RunKind::Text => run_text(w, x, o, &r.text, &r.props),
            RunKind::Break => {
                w.open0("a:br");
                rpr(w, x, o, "a:rPr", &r.props);
                w.close("a:br");
            }
            RunKind::Field { field } => {
                let id = crate::tables::guid(&format!("{}:{index}:{ri}:{field}", o.name));
                let ty = (!field.is_empty() && field.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).then_some(field.as_str());
                w.open("a:fld", A::new().a("id", id).o("type", ty));
                rpr(w, x, o, "a:rPr", &r.props);
                w.elt("a:t", &r.text);
                w.close("a:fld");
            }
            RunKind::Math { omml } => {
                if math && !omml.is_empty() {
                    w.open("a14:m", A::new().a("xmlns:a14", crate::opc::NS_A14));
                    w.raw(omml);
                    w.close("a14:m");
                } else {
                    run_text(w, x, o, &r.text, &r.props);
                }
            }
        }
    }
    if p.end_props != RunProps::default() {
        rpr(w, x, o, "a:endParaRPr", &p.end_props);
    }
    w.close("a:p");
}

pub fn has_math(tb: &TextBody) -> bool {
    tb.paragraphs.iter().flat_map(|p| &p.runs).any(|r| matches!(&r.kind, RunKind::Math { omml } if !omml.is_empty()))
}
