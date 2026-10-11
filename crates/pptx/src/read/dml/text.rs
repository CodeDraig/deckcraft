//! Read DrawingML text properties, paragraphs and hyperlinks.

use super::super::{Imp, Part};
use super::{FILL_NAMES, ang, color, effects, fill_el, line, media_ref, pct, pt};
use crate::xml::El;
use deckcraft_model::text::{
    Action, Align, Anchor, AutoFit, BodyProps, Bullet, Caps, Hyperlink, LevelStyle, ListStyle, ParaProps, Paragraph, Run, RunKind, RunProps, Spacing,
    Strike, TabStop, TextBody, TextDir,
};

pub fn body_pr(b: &El) -> BodyProps {
    let mut p = BodyProps {
        inset_l: pt(b, "lIns"),
        inset_t: pt(b, "tIns"),
        inset_r: pt(b, "rIns"),
        inset_b: pt(b, "bIns"),
        anchor: b.attr("anchor").and_then(Anchor::from_xml),
        anchor_ctr: b.bool("anchorCtr"),
        wrap: b.attr("wrap").map(|w| w != "none"),
        vert: b.attr("vert").and_then(TextDir::from_xml),
        rot: ang(b, "rot"),
        columns: b.u32("numCol").map(|n| n.clamp(1, 16)),
        col_spacing: pt(b, "spcCol"),
        rtl_col: b.bool("rtlCol"),
        upright: b.bool("upright"),
        ..Default::default()
    };
    for e in b.elements() {
        match e.local() {
            "normAutofit" => {
                p.autofit = Some(AutoFit::Shrink {
                    font_scale: pct(e, "fontScale").unwrap_or(1.0).clamp(0.01, 1.0),
                    line_reduction: pct(e, "lnSpcReduction").unwrap_or(0.0).clamp(0.0, 1.0),
                })
            }
            "spAutoFit" => p.autofit = Some(AutoFit::Shape),
            "noAutofit" => p.autofit = Some(AutoFit::None),
            "prstTxWarp" => p.warp = e.attr("prst").filter(|w| *w != "textNoShape").map(String::from),
            _ => {}
        }
    }
    p
}

pub(super) fn spacing(e: &El) -> Option<Spacing> {
    if let Some(p) = e.child("spcPct") {
        return Some(Spacing::Pct(pct(p, "val").unwrap_or(1.0).clamp(0.0, 100.0)));
    }
    e.child("spcPts").map(|p| Spacing::Pts(p.f64("val").unwrap_or(0.0).clamp(0.0, 1_000_000.0) / 100.0))
}

/// `a:pPr` / `a:lvlNpPr` → (level, paragraph props, default run props).
pub fn ppr(imp: &mut Imp, part: &Part, p: &El) -> (Option<u8>, ParaProps, Option<RunProps>) {
    let mut pp = ParaProps {
        align: p.attr("algn").and_then(Align::from_xml),
        margin_left: pt(p, "marL"),
        margin_right: pt(p, "marR"),
        indent: pt(p, "indent"),
        rtl: p.bool("rtl"),
        font_align: p.attr("fontAlgn").map(String::from),
        east_asian_line_break: p.bool("eaLnBrk"),
        default_tab: pt(p, "defTabSz"),
        ..Default::default()
    };
    let lvl = p.u32("lvl").map(|l| l.min(8) as u8);
    let mut def = None;
    for e in p.elements() {
        match e.local() {
            "lnSpc" => pp.line_spacing = spacing(e),
            "spcBef" => pp.space_before = spacing(e),
            "spcAft" => pp.space_after = spacing(e),
            "buNone" => pp.bullet = Some(Bullet::None),
            "buChar" => pp.bullet = Some(Bullet::Char { char: e.attr("char").unwrap_or("•").chars().take(4).collect() }),
            "buAutoNum" => {
                pp.bullet = Some(Bullet::AutoNum {
                    scheme: e.attr("type").unwrap_or("arabicPeriod").to_string(),
                    start_at: e.u32("startAt").unwrap_or(1).clamp(1, 32767),
                })
            }
            "buBlip" => {
                if let Some(m) = e.child("blip").and_then(|b| media_ref(imp, part, b)) {
                    pp.bullet = Some(Bullet::Picture { media: m });
                }
            }
            "buFont" => pp.bullet_font = e.attr("typeface").map(String::from),
            "buClr" => pp.bullet_color = color(e),
            "buSzPct" => pp.bullet_size = pct(e, "val").map(|v| v.clamp(0.25, 4.0)),
            "tabLst" => {
                pp.tabs = Some(
                    e.children_named("tab")
                        .take(64)
                        .map(|t| TabStop { pos: pt(t, "pos").unwrap_or(0.0), align: t.attr("algn").unwrap_or("l").to_string() })
                        .collect(),
                )
            }
            "defRPr" => def = Some(rpr(imp, part, e)),
            _ => {}
        }
    }
    (lvl, pp, def)
}

pub fn hyperlink(imp: &Imp, part: &Part, h: &El) -> Option<Hyperlink> {
    let rid = h.attr("r:id").unwrap_or("");
    let rel = part.rels.get(rid);
    let action = h.attr("action").unwrap_or("");
    let a = if let Some(rest) = action.strip_prefix("ppaction://hlinkshowjump?jump=") {
        match rest {
            "nextslide" => Action::NextSlide,
            "previousslide" => Action::PreviousSlide,
            "firstslide" => Action::FirstSlide,
            "lastslide" => Action::LastSlide,
            "endshow" => Action::EndShow,
            "lastslideviewed" => Action::LastViewed,
            _ => return None,
        }
    } else if action.starts_with("ppaction://hlinksldjump") {
        let slide = rel.and_then(|r| imp.slide_by_part.get(&crate::opc::norm(&r.target)).copied())?;
        Action::Slide { slide }
    } else if let Some(q) = action.strip_prefix("ppaction://customshow?") {
        let id = q.split('&').find_map(|kv| kv.strip_prefix("id=")).unwrap_or("0");
        Action::CustomShow { name: imp.custom_show_names.get(id).cloned().unwrap_or_else(|| id.to_string()) }
    } else if action.starts_with("ppaction://program") {
        Action::Program { path: rel.map(|r| r.target.clone()).unwrap_or_default() }
    } else if action.starts_with("ppaction://media") {
        Action::PlayMedia
    } else if action.is_empty() || action.starts_with("ppaction://hlinkfile") {
        let r = rel?;
        if r.target.is_empty() {
            return None;
        }
        if !r.external
            && let Some(s) = imp.slide_by_part.get(&crate::opc::norm(&r.target))
        {
            Action::Slide { slide: *s }
        } else {
            Action::Url { url: r.target.clone() }
        }
    } else {
        return None;
    };
    Some(Hyperlink { action: a, tooltip: h.attr("tooltip").unwrap_or("").to_string(), highlight_click: h.bool("highlightClick").unwrap_or(false) })
}

pub fn rpr(imp: &mut Imp, part: &Part, r: &El) -> RunProps {
    let mut p = RunProps {
        lang: r.attr("lang").map(String::from),
        size: r.f64("sz").map(|s| (s / 100.0).clamp(1.0, 4000.0)),
        bold: r.bool("b"),
        italic: r.bool("i"),
        underline: r.attr("u").map(String::from),
        strike: r.attr("strike").map(|s| match s {
            "sngStrike" => Strike::Single,
            "dblStrike" => Strike::Double,
            _ => Strike::None,
        }),
        baseline: pct(r, "baseline").map(|b| b.clamp(-10.0, 10.0)),
        caps: r.attr("cap").map(|c| match c {
            "small" => Caps::Small,
            "all" => Caps::All,
            _ => Caps::None,
        }),
        spacing: r.f64("spc").map(|s| (s / 100.0).clamp(-4000.0, 4000.0)),
        kern: r.f64("kern").map(|k| (k / 100.0).clamp(0.0, 4000.0)),
        no_proof: r.bool("noProof"),
        ..Default::default()
    };
    for e in r.elements() {
        match e.local() {
            "ln" => p.outline = Some(line(imp, part, e)),
            n if FILL_NAMES.contains(&n) => p.fill = fill_el(imp, part, e),
            "effectLst" => {
                let fx = effects(e);
                p.shadow = fx.outer_shadow;
                p.glow = fx.glow;
            }
            "highlight" => p.highlight = color(e),
            "uFill" => p.underline_color = e.child("solidFill").and_then(color),
            "latin" => p.font = e.attr("typeface").filter(|t| !t.is_empty()).map(String::from),
            "ea" => p.font_ea = e.attr("typeface").filter(|t| !t.is_empty()).map(String::from),
            "cs" => p.font_cs = e.attr("typeface").filter(|t| !t.is_empty()).map(String::from),
            "sym" => p.font_sym = e.attr("typeface").filter(|t| !t.is_empty()).map(String::from),
            "hlinkClick" => p.link = hyperlink(imp, part, e),
            _ => {}
        }
    }
    p
}

/// `a:lstStyle`, `p:titleStyle`, `a:defaultTextStyle`… → list style.
pub fn list_style(imp: &mut Imp, part: &Part, ls: &El) -> ListStyle {
    let mut out = ListStyle::default();
    for e in ls.elements() {
        let n = e.local();
        let Some(l) = n.strip_prefix("lvl").and_then(|r| r.strip_suffix("pPr")).and_then(|d| d.parse::<u8>().ok()) else { continue };
        if !(1..=9).contains(&l) {
            continue;
        }
        let (_, para, run) = ppr(imp, part, e);
        out.set(l - 1, LevelStyle { para, run: run.unwrap_or_default() });
    }
    out
}

pub fn text_body(imp: &mut Imp, part: &Part, tx: &El) -> TextBody {
    let body = tx.child("bodyPr").map(body_pr).unwrap_or_default();
    let list_style = tx.child("lstStyle").map(|l| list_style(imp, part, l)).unwrap_or_default();
    let paragraphs = tx.children_named("p").take(100_000).map(|p| paragraph(imp, part, p)).collect();
    TextBody { body, list_style, paragraphs }
}

pub fn paragraph(imp: &mut Imp, part: &Part, p: &El) -> Paragraph {
    let mut para = Paragraph::default();
    for e in p.elements() {
        match e.local() {
            "pPr" => {
                let (lvl, pp, _) = ppr(imp, part, e);
                para.level = lvl.unwrap_or(0);
                para.props = pp;
            }
            "r" => {
                let props = e.child("rPr").map(|r| rpr(imp, part, r)).unwrap_or_default();
                let text: String = e.child("t").map(|t| t.text()).unwrap_or_default();
                // Vertical tab inside text is a soft break in some writers.
                for (i, piece) in text.split('\u{b}').enumerate() {
                    if i > 0 {
                        para.runs.push(Run { text: String::new(), props: props.clone(), kind: RunKind::Break });
                    }
                    if !piece.is_empty() {
                        para.runs.push(Run { text: piece.to_string(), props: props.clone(), kind: RunKind::Text });
                    }
                }
            }
            "br" => {
                let props = e.child("rPr").map(|r| rpr(imp, part, r)).unwrap_or_default();
                para.runs.push(Run { text: String::new(), props, kind: RunKind::Break });
            }
            "fld" => {
                let props = e.child("rPr").map(|r| rpr(imp, part, r)).unwrap_or_default();
                let text = e.child("t").map(|t| t.text()).unwrap_or_default();
                para.runs.push(Run { text, props, kind: RunKind::Field { field: e.attr("type").unwrap_or("").to_string() } });
            }
            "m" => {
                // Office Math (a14:m) — keep the OMML and its plain text.
                let mut text = String::new();
                let mut ts = vec![];
                e.find_all("t", &mut ts);
                for t in ts {
                    text.push_str(&t.text());
                }
                let omml = e.elements().map(|c| c.to_xml()).collect::<String>();
                para.runs.push(Run { text, props: RunProps::default(), kind: RunKind::Math { omml } });
            }
            "endParaRPr" => para.end_props = rpr(imp, part, e),
            _ => {}
        }
    }
    para
}
