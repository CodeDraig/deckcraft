//! DrawingML writing: colours, fills, lines, effects, transforms and text.

mod text;
pub use text::{has_math, hyperlink, list_style, text_body};

use deckcraft_geom::{Xfrm, pt_to_emu};
use deckcraft_model::style::{ColorBase, ColorRef, Compound, Dash, Effects, Fill, GradientShape, Line, LineCap, LineJoin, PictureFill, PictureMode};

use super::{Exp, Out};
use crate::xml::{A, W};

pub fn emu(pt: f64) -> i64 {
    pt_to_emu(pt).clamp(-27_273_042_316_900, 27_273_042_316_900)
}

/// Positive coordinate extent (ST_PositiveCoordinate).
pub fn emu_pos(pt: f64) -> i64 {
    emu(pt).max(0)
}

/// Fraction → 1/100 000.
pub fn pct(v: f64) -> i64 {
    if v.is_finite() { (v * 100_000.0).round().clamp(-2.0e9, 2.0e9) as i64 } else { 0 }
}

/// Degrees → 1/60 000 degree, normalised to 0..360.
pub fn ang(deg: f64) -> i64 {
    if !deg.is_finite() {
        return 0;
    }
    let d = deg.rem_euclid(360.0);
    ((d * 60_000.0).round() as i64).clamp(0, 21_599_999)
}

pub fn color(w: &mut W, c: &ColorRef) {
    let (tag, a) = match &c.base {
        ColorBase::Rgb { rgb } => ("a:srgbClr", A::new().a("val", rgb.hex())),
        ColorBase::Scheme { slot } => ("a:schemeClr", A::new().a("val", slot.xml_name())),
        ColorBase::Preset { name } => ("a:prstClr", A::new().a("val", if name.is_empty() { "black" } else { name })),
        ColorBase::System { name, last } => {
            ("a:sysClr", A::new().a("val", if name.is_empty() { "windowText" } else { name }).a("lastClr", last.hex()))
        }
    };
    let rgb_alpha = match &c.base {
        ColorBase::Rgb { rgb } if rgb.a < 255 && !c.mods.iter().any(|m| matches!(m, deckcraft_color::ColorTransform::Alpha(_))) => {
            Some((rgb.a as i64 * 100_000 / 255) as i32)
        }
        _ => None,
    };
    if c.mods.is_empty() && rgb_alpha.is_none() {
        w.empty(tag, a);
        return;
    }
    w.open(tag, a);
    for m in &c.mods {
        let name = format!("a:{}", m.xml_name());
        match m.value() {
            Some(v) => w.val(&name, v),
            None => w.empty0(&name),
        }
    }
    if let Some(al) = rgb_alpha {
        w.val("a:alpha", al);
    }
    w.close(tag);
}

pub fn solid(w: &mut W, c: &ColorRef) {
    w.open0("a:solidFill");
    color(w, c);
    w.close("a:solidFill");
}

/// Write a fill. `Fill::Background` (only meaningful on `p:sp`) writes nothing.
pub fn fill(w: &mut W, x: &mut Exp, o: &mut Out, f: &Fill) {
    match f {
        Fill::None => w.empty0("a:noFill"),
        Fill::Solid { color: c } => solid(w, c),
        Fill::Gradient(g) => {
            w.open("a:gradFill", A::new().a("rotWithShape", if g.rotate_with_shape { "1" } else { "0" }));
            w.open0("a:gsLst");
            let mut stops = g.stops.clone();
            if stops.is_empty() {
                stops.push(deckcraft_model::style::GradientStop { pos: 0.0, color: ColorRef::rgb(deckcraft_color::Rgba::WHITE) });
            }
            if stops.len() == 1 {
                let mut s = stops[0].clone();
                s.pos = 1.0;
                stops.push(s);
            }
            for s in &stops {
                w.open("a:gs", A::new().a("pos", pct(s.pos.clamp(0.0, 1.0))));
                color(w, &s.color);
                w.close("a:gs");
            }
            w.close("a:gsLst");
            match &g.shape {
                GradientShape::Linear { angle, scaled } => {
                    w.empty("a:lin", A::new().a("ang", ang(*angle)).a("scaled", if *scaled { "1" } else { "0" }))
                }
                GradientShape::Path { path, focus } => {
                    let p = match path.as_str() {
                        "rect" | "shape" | "circle" => path.as_str(),
                        _ => "circle",
                    };
                    w.open("a:path", A::new().a("path", p));
                    w.empty("a:fillToRect", A::new().a("l", pct(focus[0])).a("t", pct(focus[1])).a("r", pct(focus[2])).a("b", pct(focus[3])));
                    w.close("a:path");
                }
            }
            w.close("a:gradFill");
        }
        Fill::Picture(pf) => blip_fill(w, x, o, "a:blipFill", pf),
        Fill::Pattern(p) => {
            w.open("a:pattFill", A::new().a("prst", if p.preset.is_empty() { "pct5" } else { &p.preset }));
            w.open0("a:fgClr");
            color(w, &p.fg);
            w.close("a:fgClr");
            w.open0("a:bgClr");
            color(w, &p.bg);
            w.close("a:bgClr");
            w.close("a:pattFill");
        }
        Fill::Group => w.empty0("a:grpFill"),
        Fill::Background => {}
    }
}

/// `a:blipFill` / `p:blipFill`.
pub fn blip_fill(w: &mut W, x: &mut Exp, o: &mut Out, tag: &str, pf: &PictureFill) {
    let rid = x.media_rel(o, pf.media, "image").unwrap_or_else(|| x.placeholder_rel(o));
    w.open(tag, A::new().a("rotWithShape", "1"));
    let adj = &pf.adjust;
    let has_kids =
        pf.alpha.is_some() || adj.brightness != 0.0 || adj.contrast != 0.0 || adj.grayscale || adj.duotone.is_some() || adj.clear_color.is_some();
    let a = A::new().a("r:embed", &rid);
    if has_kids {
        w.open("a:blip", a);
        if let Some(c) = adj.clear_color {
            w.open0("a:clrChange");
            w.open0("a:clrFrom");
            color(w, &ColorRef::rgb(c));
            w.close("a:clrFrom");
            w.open0("a:clrTo");
            color(w, &ColorRef::rgb(c).with(deckcraft_color::ColorTransform::Alpha(0)));
            w.close("a:clrTo");
            w.close("a:clrChange");
        }
        if let Some(al) = pf.alpha {
            w.empty("a:alphaModFix", A::new().a("amt", pct(al.clamp(0.0, 1.0))));
        }
        if let Some((d, l)) = &adj.duotone {
            w.open0("a:duotone");
            color(w, d);
            color(w, l);
            w.close("a:duotone");
        }
        if adj.grayscale {
            w.empty0("a:grayscl");
        }
        if adj.brightness != 0.0 || adj.contrast != 0.0 {
            w.empty("a:lum", A::new().a("bright", pct(adj.brightness.clamp(-1.0, 1.0))).a("contrast", pct(adj.contrast.clamp(-1.0, 1.0))));
        }
        w.close("a:blip");
    } else {
        w.empty("a:blip", a);
    }
    let c = pf.crop;
    if c.iter().any(|v| *v != 0.0) {
        w.empty("a:srcRect", A::new().a("l", pct(c[0])).a("t", pct(c[1])).a("r", pct(c[2])).a("b", pct(c[3])));
    }
    match &pf.mode {
        PictureMode::Stretch { fill_rect: r } => {
            w.open0("a:stretch");
            if r.iter().any(|v| *v != 0.0) {
                w.empty("a:fillRect", A::new().a("l", pct(r[0])).a("t", pct(r[1])).a("r", pct(r[2])).a("b", pct(r[3])));
            } else {
                w.empty0("a:fillRect");
            }
            w.close("a:stretch");
        }
        PictureMode::Tile { tx, ty, sx, sy, flip, align } => {
            let flip = match flip.as_str() {
                "x" | "y" | "xy" => flip.as_str(),
                _ => "none",
            };
            let algn = match align.as_str() {
                "tl" | "t" | "tr" | "l" | "ctr" | "r" | "bl" | "b" | "br" => align.as_str(),
                _ => "tl",
            };
            w.empty("a:tile", A::new().a("tx", emu(*tx)).a("ty", emu(*ty)).a("sx", pct(*sx)).a("sy", pct(*sy)).a("flip", flip).a("algn", algn));
        }
    }
    w.close(tag);
}

pub fn line(w: &mut W, x: &mut Exp, o: &mut Out, tag: &str, l: &Line) {
    let a = A::new()
        .o("w", l.width.map(|v| emu(v.clamp(0.0, 1584.0))))
        .o(
            "cap",
            l.cap.map(|c| match c {
                LineCap::Round => "rnd",
                LineCap::Square => "sq",
                LineCap::Flat => "flat",
            }),
        )
        .o("cmpd", l.compound.map(Compound::xml));
    w.open(tag, a);
    if let Some(f) = &l.fill {
        match f {
            Fill::None | Fill::Solid { .. } | Fill::Gradient(_) | Fill::Pattern(_) => fill(w, x, o, f),
            _ => {}
        }
    }
    match &l.dash {
        Some(Dash::Preset { name }) => {
            let n = if Dash::PRESETS.contains(&name.as_str()) { name.as_str() } else { "solid" };
            w.val("a:prstDash", n);
        }
        Some(Dash::Custom { pattern }) if !pattern.is_empty() => {
            w.open0("a:custDash");
            for (d, s) in pattern.iter().take(64) {
                w.empty("a:ds", A::new().a("d", pct(*d).max(0)).a("sp", pct(*s).max(0)));
            }
            w.close("a:custDash");
        }
        _ => {}
    }
    match l.join {
        Some(LineJoin::Round) => w.empty0("a:round"),
        Some(LineJoin::Bevel) => w.empty0("a:bevel"),
        Some(LineJoin::Miter) => w.empty0("a:miter"),
        None => {}
    }
    for (t, e) in [("a:headEnd", &l.head), ("a:tailEnd", &l.tail)] {
        if let Some(e) = e {
            let kind = match e.kind.as_str() {
                "triangle" | "stealth" | "diamond" | "oval" | "arrow" => e.kind.as_str(),
                _ => "none",
            };
            let sz = |s: &str| match s {
                "sm" | "lg" => s.to_string(),
                _ => "med".to_string(),
            };
            w.empty(t, A::new().a("type", kind).a("w", sz(&e.w)).a("len", sz(&e.len)));
        }
    }
    w.close(tag);
}

fn shadow(w: &mut W, tag: &str, s: &deckcraft_model::style::Shadow) {
    let mut a = A::new().a("blurRad", emu(s.blur).max(0)).a("dist", emu(s.dist).max(0)).a("dir", ang(s.dir));
    if tag == "a:outerShdw" {
        if s.sx != 1.0 {
            a = a.a("sx", pct(s.sx));
        }
        if s.sy != 1.0 {
            a = a.a("sy", pct(s.sy));
        }
        if s.kx != 0.0 {
            a = a.a("kx", (s.kx.clamp(-89.0, 89.0) * 60_000.0).round() as i64);
        }
        if s.ky != 0.0 {
            a = a.a("ky", (s.ky.clamp(-89.0, 89.0) * 60_000.0).round() as i64);
        }
        let algn = match s.align.as_str() {
            "tl" | "t" | "tr" | "l" | "ctr" | "r" | "bl" | "b" | "br" => s.align.as_str(),
            _ => "b",
        };
        a = a.a("algn", algn).a("rotWithShape", if s.rotate_with_shape { "1" } else { "0" });
    }
    w.open(tag, a);
    color(w, &s.color);
    w.close(tag);
}

pub fn effects(w: &mut W, fx: &Effects) {
    w.open0("a:effectLst");
    if let Some(g) = &fx.glow {
        w.open("a:glow", A::new().a("rad", emu(g.radius).max(0)));
        color(w, &g.color);
        w.close("a:glow");
    }
    if let Some(s) = &fx.inner_shadow {
        shadow(w, "a:innerShdw", s);
    }
    if let Some(s) = &fx.outer_shadow {
        shadow(w, "a:outerShdw", s);
    }
    if let Some(r) = &fx.reflection {
        w.empty(
            "a:reflection",
            A::new()
                .a("blurRad", emu(r.blur).max(0))
                .a("stA", pct(r.start_alpha.clamp(0.0, 1.0)))
                .a("endA", pct(r.end_alpha.clamp(0.0, 1.0)))
                .a("endPos", pct(r.end_pos.clamp(0.0, 1.0)))
                .a("dist", emu(r.dist).max(0))
                .a("dir", 5_400_000)
                .a("sy", -100_000)
                .a("algn", "bl")
                .a("rotWithShape", "0"),
        );
    }
    if let Some(r) = fx.soft_edge {
        w.empty("a:softEdge", A::new().a("rad", emu(r).max(0)));
    }
    w.close("a:effectLst");
}

pub fn xfrm(w: &mut W, tag: &str, x: &Xfrm, child: Option<&Xfrm>) {
    let x = if x.is_finite() { *x } else { Xfrm::default() };
    let rot = ang(x.rot);
    w.open(tag, A::new().o("rot", (rot != 0).then_some(rot)).t("flipH", x.flip_h).t("flipV", x.flip_v));
    w.empty("a:off", A::new().a("x", emu(x.x)).a("y", emu(x.y)));
    w.empty("a:ext", A::new().a("cx", emu_pos(x.w)).a("cy", emu_pos(x.h)));
    if let Some(c) = child {
        let c = if c.is_finite() { *c } else { Xfrm::default() };
        w.empty("a:chOff", A::new().a("x", emu(c.x)).a("y", emu(c.y)));
        w.empty("a:chExt", A::new().a("cx", emu_pos(c.w)).a("cy", emu_pos(c.h)));
    }
    w.close(tag);
}

// ---------------------------------------------------------------------------------------------
// Text
