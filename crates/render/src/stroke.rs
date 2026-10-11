//! Stroke outlines, dash patterns and line-end geometry.

use super::{color, scale_of};
use deckcraft_color::Rgba;
use deckcraft_geom::preset::Geometry;
use deckcraft_model::resolve::Ctx;
use deckcraft_model::style::{Fill, Line};
use kurbo::{Affine, BezPath, PathEl, Point, Shape as _, Vec2};
use vello_cpu::{RenderContext, peniko};

pub(super) fn stroke(ctx: &mut RenderContext, rctx: &Ctx, line: &Line, ph: Option<Rgba>, path: &BezPath, m: Affine, lw: f64) {
    use deckcraft_model::style::{LineCap, LineJoin};
    let cap = match line.cap.unwrap_or_default() {
        LineCap::Flat => kurbo::Cap::Butt,
        LineCap::Round => kurbo::Cap::Round,
        LineCap::Square => kurbo::Cap::Square,
    };
    let join = match line.join.unwrap_or_default() {
        LineJoin::Round => kurbo::Join::Round,
        LineJoin::Bevel => kurbo::Join::Bevel,
        LineJoin::Miter => kurbo::Join::Miter,
    };
    // Hairlines stay visible at any zoom.
    let min_w = 0.5 / scale_of(m).max(1e-6);
    let mut sk = kurbo::Stroke::new(lw.max(min_w)).with_caps(cap).with_join(join);
    if let Some(d) = &line.dash {
        let pat: Vec<f64> = d.pattern().iter().map(|v| v * lw.max(0.75)).collect();
        if !pat.is_empty() && pat.iter().all(|v| *v > 0.0) {
            sk = sk.with_dashes(0.0, pat);
        }
    }
    ctx.set_transform(m);
    match &line.fill {
        Some(Fill::Solid { color: c }) => {
            ctx.set_paint(color(rctx.color(c, ph), 1.0));
        }
        Some(Fill::Gradient(g)) => {
            let c = g.stops.first().map(|s| rctx.color(&s.color, ph)).unwrap_or(Rgba::BLACK);
            ctx.set_paint(color(c, 1.0));
        }
        _ => ctx.set_paint(peniko::Color::from_rgba8(0, 0, 0, 255)),
    }
    ctx.set_stroke(sk);
    ctx.stroke_path(path);
}

pub(super) fn end_points(path: &BezPath) -> Option<((Point, Point), (Point, Point))> {
    let els: Vec<PathEl> = path.elements().to_vec();
    let mut pts: Vec<Point> = vec![];
    for e in &els {
        match e {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => pts.push(*p),
            PathEl::QuadTo(a, b) => {
                pts.push(*a);
                pts.push(*b);
            }
            PathEl::CurveTo(a, b, c) => {
                pts.push(*a);
                pts.push(*b);
                pts.push(*c);
            }
            PathEl::ClosePath => {}
        }
    }
    if pts.len() < 2 {
        return None;
    }
    let first = *pts.first()?;
    let second = pts.iter().skip(1).find(|p| (**p - first).hypot() > 1e-6).copied().unwrap_or(*pts.get(1)?);
    let last = *pts.last()?;
    let prev = pts.iter().rev().skip(1).find(|p| (**p - last).hypot() > 1e-6).copied().unwrap_or(*pts.get(pts.len() - 2)?);
    Some(((first, second), (last, prev)))
}

pub(super) fn arrowheads(ctx: &mut RenderContext, rctx: &Ctx, line: &Line, ph: Option<Rgba>, geo: &Geometry, m: Affine, lw: f64) {
    let Some(sp) = geo.paths.iter().find(|p| p.stroke) else { return };
    let Some(((a, a2), (b, b2))) = end_points(&sp.path) else { return };
    let col = match &line.fill {
        Some(Fill::Solid { color: c }) => rctx.color(c, ph),
        _ => Rgba::BLACK,
    };
    let lw = lw.max(0.75);
    for (end, tip, from) in [(&line.head, a, a2), (&line.tail, b, b2)] {
        let Some(e) = end else { continue };
        if e.kind == "none" {
            continue;
        }
        let sz = |s: &str| match s {
            "sm" => 2.0,
            "lg" => 5.0,
            _ => 3.0,
        };
        let (wl, ll) = (sz(&e.w) * lw, sz(&e.len) * lw);
        let dir = (tip - from).normalize();
        if !dir.x.is_finite() {
            continue;
        }
        let n = Vec2::new(-dir.y, dir.x);
        let base = tip - dir * ll;
        let mut p = BezPath::new();
        match e.kind.as_str() {
            "oval" => {
                let c = kurbo::Ellipse::new(tip, (ll / 2.0, wl / 2.0), dir.y.atan2(dir.x));
                p = c.to_path(0.05);
            }
            "diamond" => {
                p.move_to(tip + dir * ll / 2.0);
                p.line_to(tip + n * wl / 2.0);
                p.line_to(tip - dir * ll / 2.0);
                p.line_to(tip - n * wl / 2.0);
                p.close_path();
            }
            "stealth" => {
                p.move_to(tip);
                p.line_to(base + n * wl / 2.0);
                p.line_to(tip - dir * ll * 0.6);
                p.line_to(base - n * wl / 2.0);
                p.close_path();
            }
            "arrow" => {
                let mut o = BezPath::new();
                o.move_to(base + n * wl / 2.0);
                o.line_to(tip);
                o.line_to(base - n * wl / 2.0);
                ctx.set_transform(m);
                ctx.set_paint(color(col, 1.0));
                ctx.set_stroke(kurbo::Stroke::new(lw).with_join(kurbo::Join::Miter));
                ctx.stroke_path(&o);
                continue;
            }
            _ => {
                p.move_to(tip);
                p.line_to(base + n * wl / 2.0);
                p.line_to(base - n * wl / 2.0);
                p.close_path();
            }
        }
        ctx.set_transform(m);
        ctx.set_paint(color(col, 1.0));
        ctx.fill_path(&p);
    }
}
