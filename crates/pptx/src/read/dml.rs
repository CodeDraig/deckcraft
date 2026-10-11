//! DrawingML reading: colours, fills, lines, effects, transforms and text.

mod text;
pub use text::{hyperlink, list_style, text_body};

use deckcraft_color::{ColorTransform, Rgba, SchemeSlot};
use deckcraft_geom::{Xfrm, emu_to_pt};
use deckcraft_model::style::{
    ColorBase, ColorRef, Compound, Dash, Effects, Fill, Glow, Gradient, GradientShape, GradientStop, Line, LineCap, LineEnd, LineJoin, PatternFill,
    PictureFill, PictureMode, Reflection, Shadow,
};

use super::{Imp, Part};
use crate::xml::El;

pub fn pt(e: &El, name: &str) -> Option<f64> {
    e.i64(name).map(emu_to_pt)
}

/// 1/100 000 → fraction.
pub fn pct(e: &El, name: &str) -> Option<f64> {
    e.f64(name).map(|v| v / 100_000.0)
}

/// 1/60 000 degree → degrees.
pub fn ang(e: &El, name: &str) -> Option<f64> {
    e.f64(name).map(|v| v / 60_000.0)
}

fn lin_to_srgb(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// A colour element (`srgbClr`, `schemeClr`, …) with its transforms.
pub fn color_el(c: &El) -> Option<ColorRef> {
    let base = match c.local() {
        "srgbClr" => ColorBase::Rgb { rgb: Rgba::from_hex(c.attr("val").unwrap_or("000000")).unwrap_or(Rgba::BLACK) },
        "schemeClr" => ColorBase::Scheme { slot: c.attr("val").and_then(SchemeSlot::from_xml).unwrap_or(SchemeSlot::Tx1) },
        "sysClr" => ColorBase::System {
            name: c.attr("val").unwrap_or("windowText").to_string(),
            last: c.attr("lastClr").and_then(Rgba::from_hex).unwrap_or(if c.attr("val") == Some("window") { Rgba::WHITE } else { Rgba::BLACK }),
        },
        "prstClr" => ColorBase::Preset { name: c.attr("val").unwrap_or("black").to_string() },
        "scrgbClr" => {
            let f = |n: &str| lin_to_srgb(c.f64(n).unwrap_or(0.0) / 100_000.0);
            ColorBase::Rgb { rgb: Rgba::from_f64(f("r"), f("g"), f("b"), 1.0) }
        }
        "hslClr" => {
            let h = c.f64("hue").unwrap_or(0.0) / 60_000.0;
            let s = c.f64("sat").unwrap_or(0.0) / 100_000.0;
            let l = c.f64("lum").unwrap_or(0.0) / 100_000.0;
            ColorBase::Rgb { rgb: Rgba::from_hsl(h, s, l, 255) }
        }
        _ => return None,
    };
    let mods = c.elements().take(32).filter_map(|m| ColorTransform::from_xml(m.local(), m.i32("val"))).collect();
    Some(ColorRef { base, mods })
}

/// The first colour child of a container (`solidFill`, `fgClr`, `lnRef`…).
pub fn color(container: &El) -> Option<ColorRef> {
    container.elements().find_map(color_el)
}

const FILL_NAMES: [&str; 6] = ["noFill", "solidFill", "gradFill", "blipFill", "pattFill", "grpFill"];

/// The fill child of `parent`, if any.
pub fn fill_of(imp: &mut Imp, part: &Part, parent: &El) -> Option<Fill> {
    parent.elements().find(|e| FILL_NAMES.contains(&e.local())).and_then(|f| fill_el(imp, part, f))
}

pub fn fill_el(imp: &mut Imp, part: &Part, f: &El) -> Option<Fill> {
    Some(match f.local() {
        "noFill" => Fill::None,
        "solidFill" => Fill::Solid { color: color(f).unwrap_or(ColorRef::rgb(Rgba::BLACK)) },
        "gradFill" => Fill::Gradient(gradient(f)),
        "blipFill" => match blip_fill(imp, part, f) {
            Some(p) => Fill::Picture(p),
            None => Fill::None,
        },
        "pattFill" => Fill::Pattern(PatternFill {
            preset: f.attr("prst").unwrap_or("pct5").to_string(),
            fg: f.child("fgClr").and_then(color).unwrap_or(ColorRef::rgb(Rgba::BLACK)),
            bg: f.child("bgClr").and_then(color).unwrap_or(ColorRef::rgb(Rgba::WHITE)),
        }),
        "grpFill" => Fill::Group,
        _ => return None,
    })
}

pub fn gradient(f: &El) -> Gradient {
    let mut stops: Vec<GradientStop> = f
        .child("gsLst")
        .map(|l| {
            l.children_named("gs")
                .take(256)
                .map(|g| GradientStop { pos: pct(g, "pos").unwrap_or(0.0).clamp(0.0, 1.0), color: color(g).unwrap_or(ColorRef::rgb(Rgba::BLACK)) })
                .collect()
        })
        .unwrap_or_default();
    stops.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    let shape = if let Some(l) = f.child("lin") {
        GradientShape::Linear { angle: ang(l, "ang").unwrap_or(0.0), scaled: l.bool("scaled").unwrap_or(false) }
    } else if let Some(p) = f.child("path") {
        let r = p.child("fillToRect");
        let g = |n: &str| r.and_then(|r| pct(r, n)).unwrap_or(0.0);
        GradientShape::Path { path: p.attr("path").unwrap_or("circle").to_string(), focus: [g("l"), g("t"), g("r"), g("b")] }
    } else {
        GradientShape::Linear { angle: 90.0, scaled: false }
    };
    Gradient { stops, shape, rotate_with_shape: f.bool("rotWithShape").unwrap_or(true) }
}

/// `blipFill` (either `a:` or `p:`) → picture fill, registering the image as media.
pub fn blip_fill(imp: &mut Imp, part: &Part, bf: &El) -> Option<PictureFill> {
    let blip = bf.child("blip");
    let media = blip.and_then(|b| media_ref(imp, part, b));
    let media = media?;
    let mut pf = PictureFill { media, ..Default::default() };
    if let Some(b) = blip {
        for e in b.elements() {
            match e.local() {
                "alphaModFix" => pf.alpha = Some(pct(e, "amt").unwrap_or(1.0).clamp(0.0, 1.0)),
                "lum" => {
                    pf.adjust.brightness = pct(e, "bright").unwrap_or(0.0).clamp(-1.0, 1.0);
                    pf.adjust.contrast = pct(e, "contrast").unwrap_or(0.0).clamp(-1.0, 1.0);
                }
                "grayscl" => pf.adjust.grayscale = true,
                "duotone" => {
                    let cs: Vec<ColorRef> = e.elements().filter_map(color_el).collect();
                    if let [a, b] = cs.as_slice() {
                        pf.adjust.duotone = Some((a.clone(), b.clone()));
                    }
                }
                "clrChange" => {
                    if let Some(ColorRef { base: ColorBase::Rgb { rgb }, .. }) = e.child("clrFrom").and_then(color) {
                        pf.adjust.clear_color = Some(rgb);
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(r) = bf.child("srcRect") {
        pf.crop = [pct(r, "l").unwrap_or(0.0), pct(r, "t").unwrap_or(0.0), pct(r, "r").unwrap_or(0.0), pct(r, "b").unwrap_or(0.0)]
            .map(|v| v.clamp(-10.0, 1.0));
    }
    if let Some(t) = bf.child("tile") {
        pf.mode = PictureMode::Tile {
            tx: pt(t, "tx").unwrap_or(0.0),
            ty: pt(t, "ty").unwrap_or(0.0),
            sx: pct(t, "sx").unwrap_or(1.0),
            sy: pct(t, "sy").unwrap_or(1.0),
            flip: t.attr("flip").unwrap_or("none").to_string(),
            align: t.attr("algn").unwrap_or("tl").to_string(),
        };
    } else {
        let r = bf.path(&["stretch", "fillRect"]);
        let g = |n: &str| r.and_then(|r| pct(r, n)).unwrap_or(0.0).clamp(-10.0, 1.0);
        pf.mode = PictureMode::Stretch { fill_rect: [g("l"), g("t"), g("r"), g("b")] };
    }
    Some(pf)
}

/// Media for an element with `r:embed` (or `r:link`).
pub fn media_ref(imp: &mut Imp, part: &Part, b: &El) -> Option<deckcraft_model::MediaId> {
    if let Some(id) = b.attr("r:embed").filter(|s| !s.is_empty())
        && let Some(r) = part.rels.get(id)
    {
        if r.external {
            return Some(imp.media_link(&r.target));
        }
        let t = r.target.clone();
        if let Some(m) = imp.media_for(&t) {
            return Some(m);
        }
    }
    if let Some(id) = b.attr("r:link").filter(|s| !s.is_empty())
        && let Some(r) = part.rels.get(id)
    {
        if r.external {
            return Some(imp.media_link(&r.target));
        }
        let t = r.target.clone();
        return imp.media_for(&t);
    }
    None
}

pub fn line(imp: &mut Imp, part: &Part, ln: &El) -> Line {
    let mut l = Line { width: pt(ln, "w").map(|w| w.clamp(0.0, 2000.0)), ..Default::default() };
    l.cap = ln.attr("cap").map(|c| match c {
        "rnd" => LineCap::Round,
        "sq" => LineCap::Square,
        _ => LineCap::Flat,
    });
    l.compound = ln.attr("cmpd").map(Compound::from_xml);
    l.fill = fill_of(imp, part, ln);
    for e in ln.elements() {
        match e.local() {
            "prstDash" => l.dash = Some(Dash::Preset { name: e.attr("val").unwrap_or("solid").to_string() }),
            "custDash" => {
                let pattern = e.children_named("ds").take(64).map(|d| (pct(d, "d").unwrap_or(1.0), pct(d, "sp").unwrap_or(1.0))).collect();
                l.dash = Some(Dash::Custom { pattern });
            }
            "round" => l.join = Some(LineJoin::Round),
            "bevel" => l.join = Some(LineJoin::Bevel),
            "miter" => l.join = Some(LineJoin::Miter),
            "headEnd" => l.head = line_end(e),
            "tailEnd" => l.tail = line_end(e),
            _ => {}
        }
    }
    l
}

fn line_end(e: &El) -> Option<LineEnd> {
    let kind = e.attr("type").unwrap_or("none");
    Some(LineEnd { kind: kind.to_string(), w: e.attr("w").unwrap_or("med").to_string(), len: e.attr("len").unwrap_or("med").to_string() })
}

fn shadow(e: &El, inner: bool) -> Shadow {
    Shadow {
        color: color(e).unwrap_or(ColorRef::rgb(Rgba::BLACK)),
        blur: pt(e, "blurRad").unwrap_or(0.0).max(0.0),
        dist: pt(e, "dist").unwrap_or(0.0),
        dir: ang(e, "dir").unwrap_or(0.0),
        inner,
        sx: pct(e, "sx").unwrap_or(1.0),
        sy: pct(e, "sy").unwrap_or(1.0),
        kx: ang(e, "kx").unwrap_or(0.0),
        ky: ang(e, "ky").unwrap_or(0.0),
        align: e.attr("algn").unwrap_or(if inner { "" } else { "b" }).to_string(),
        rotate_with_shape: e.bool("rotWithShape").unwrap_or(true),
    }
}

/// `effectLst` → effects.
pub fn effects(el: &El) -> Effects {
    let mut fx = Effects::default();
    for e in el.elements() {
        match e.local() {
            "outerShdw" => fx.outer_shadow = Some(shadow(e, false)),
            "prstShdw" => {
                let mut s = shadow(e, false);
                s.align = "b".into();
                fx.outer_shadow = Some(s);
            }
            "innerShdw" => fx.inner_shadow = Some(shadow(e, true)),
            "glow" => fx.glow = Some(Glow { color: color(e).unwrap_or(ColorRef::rgb(Rgba::BLACK)), radius: pt(e, "rad").unwrap_or(0.0).max(0.0) }),
            "softEdge" => fx.soft_edge = Some(pt(e, "rad").unwrap_or(0.0).max(0.0)),
            "reflection" => {
                fx.reflection = Some(Reflection {
                    blur: pt(e, "blurRad").unwrap_or(0.0),
                    start_alpha: pct(e, "stA").unwrap_or(1.0),
                    end_alpha: pct(e, "endA").unwrap_or(0.0),
                    end_pos: pct(e, "endPos").unwrap_or(1.0),
                    dist: pt(e, "dist").unwrap_or(0.0),
                })
            }
            _ => {}
        }
    }
    fx
}

/// `a:xfrm` → (box, child box for groups).
pub fn xfrm(x: &El) -> (Xfrm, Option<Xfrm>) {
    let off = x.child("off");
    let ext = x.child("ext");
    let g = |e: Option<&El>, n: &str| e.and_then(|e| pt(e, n)).unwrap_or(0.0);
    let mut xf = Xfrm::new(g(off, "x"), g(off, "y"), g(ext, "cx").max(0.0), g(ext, "cy").max(0.0));
    xf.rot = ang(x, "rot").unwrap_or(0.0);
    if !xf.rot.is_finite() {
        xf.rot = 0.0;
    }
    xf.flip_h = x.bool("flipH").unwrap_or(false);
    xf.flip_v = x.bool("flipV").unwrap_or(false);
    let child = match (x.child("chOff"), x.child("chExt")) {
        (Some(o), Some(e)) => Some(Xfrm::new(g(Some(o), "x"), g(Some(o), "y"), g(Some(e), "cx").max(0.0), g(Some(e), "cy").max(0.0))),
        _ => None,
    };
    (xf, child)
}

// ---------------------------------------------------------------------------------------------
// Text

#[cfg(test)]
mod tests {
    use super::text::body_pr;
    use super::*;
    use crate::xml::parse;
    use deckcraft_model::text::{Anchor, AutoFit, TextDir};

    #[test]
    fn colors_and_transforms() {
        let d = parse(
            br#"<a:solidFill xmlns:a="x"><a:schemeClr val="accent2"><a:lumMod val="75000"/><a:alpha val="50000"/></a:schemeClr></a:solidFill>"#,
        )
        .unwrap();
        let c = color(&d.root).unwrap();
        assert_eq!(c.base, ColorBase::Scheme { slot: SchemeSlot::Accent2 });
        assert_eq!(c.mods, vec![ColorTransform::LumMod(75000), ColorTransform::Alpha(50000)]);
        let d = parse(br#"<x><a:sysClr val="window" lastClr="FFFFFF"/></x>"#).unwrap();
        assert!(matches!(color(&d.root).unwrap().base, ColorBase::System { last: Rgba::WHITE, .. }));
        let d = parse(br#"<x><a:scrgbClr r="100000" g="0" b="0"/></x>"#).unwrap();
        assert_eq!(color(&d.root).unwrap().base, ColorBase::Rgb { rgb: Rgba::rgb(255, 0, 0) });
    }

    #[test]
    fn body_props() {
        let d = parse(br#"<a:bodyPr lIns="91440" anchor="ctr" wrap="none" vert="vert270" numCol="2"><a:normAutofit fontScale="62500" lnSpcReduction="20000"/></a:bodyPr>"#).unwrap();
        let b = body_pr(&d.root);
        assert_eq!(b.inset_l, Some(7.2));
        assert_eq!(b.anchor, Some(Anchor::Middle));
        assert_eq!(b.wrap, Some(false));
        assert_eq!(b.vert, Some(TextDir::Vertical270));
        assert_eq!(b.columns, Some(2));
        assert_eq!(b.autofit, Some(AutoFit::Shrink { font_scale: 0.625, line_reduction: 0.2 }));
    }
}
