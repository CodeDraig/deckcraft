//! Selection, caret and active-gesture overlays.

use super::Xf;
use crate::SlideApp;
use crate::theme::{self, Tokens};
use deckcraft_engine::ToolKind;
use deckcraft_engine::tools::box_handles;
use deckcraft_geom::{Point, Xfrm};
use egui::{Color32, CornerRadius, Pos2, Rect, Stroke, Ui, pos2};

pub(super) fn poly(p: &egui::Painter, pts: [Pos2; 4], stroke: Stroke) {
    for i in 0..4 {
        p.line_segment([pts[i], pts[(i + 1) % 4]], stroke);
    }
}

pub(super) fn overlays(app: &mut SlideApp, ui: &Ui, painter: &egui::Painter, xf: Xf, t: &Tokens) {
    let Some(st) = app.session.active() else { return };
    let sel_stroke = Stroke::new(1.0, t.selection);
    let editing = st.selection.text.clone();
    for id in &st.selection.shapes {
        let Some(sh) = st.shape(*id) else { continue };
        let x = deckcraft_engine::cmd::xfrm_of(&st.doc, &st.selection, sh);
        let a = x.affine();
        let corners = [Point::new(0.0, 0.0), Point::new(x.w, 0.0), Point::new(x.w, x.h), Point::new(0.0, x.h)].map(|q| xf.to_screen(a * q));
        if sh.is_line() {
            let (p0, p1) = (xf.to_screen(a * Point::new(0.0, 0.0)), xf.to_screen(a * Point::new(x.w, x.h)));
            let glued = match sh.kind {
                deckcraft_model::ShapeKind::Connector { start, end } => [start.is_some(), end.is_some()],
                _ => [false, false],
            };
            for (q, g) in [p0, p1].into_iter().zip(glued) {
                // Glued ends are filled with the accent, like attached connector ends.
                let fill = if g { t.accent } else { t.handle_fill };
                painter.circle(q, 4.5, fill, Stroke::new(1.0, if g { t.handle_fill } else { t.handle_stroke }));
            }
            continue;
        }
        let editing_this = editing.as_ref().is_some_and(|e| e.shape == *id && !e.notes);
        if editing_this {
            // Dashed box while editing text.
            for i in 0..4 {
                let (s, e) = (corners[i], corners[(i + 1) % 4]);
                painter.add(egui::Shape::dashed_line(&[s, e], Stroke::new(1.0, t.selection), 4.0, 3.0));
            }
        } else {
            poly(painter, corners, sel_stroke);
        }
        if st.selection.shapes.len() == 1 || !editing_this {
            let (hs, rot) = box_handles(&x);
            let rot_s = xf.to_screen(rot);
            if !editing_this && sh.ph.as_ref().is_none_or(|_| true) {
                let top_mid = xf.to_screen(hs[1]);
                painter.line_segment([top_mid, rot_s], sel_stroke);
                rotate_handle(painter, rot_s, t);
            }
            for h in hs {
                painter.circle(xf.to_screen(h), 4.5, t.handle_fill, Stroke::new(1.0, t.handle_stroke));
            }
            let geo = deckcraft_render::shape_geometry(sh, x.w, x.h);
            for hd in &geo.handles {
                let q = xf.to_screen(a * hd.pos);
                painter.circle(q, 4.5, Color32::from_rgb(0xFF, 0xD2, 0x3F), Stroke::new(1.0, Color32::from_rgb(0x8A, 0x6A, 0x00)));
            }
        }
    }
    // Caret / text selection.
    if let Some(ts) = &editing
        && !ts.notes
        && ts.cell.is_none()
        && let Some(sh) = st.shape(ts.shape)
        && let Some(l) = deckcraft_engine::cmd::text::layout_for(st, ts)
    {
        let x = deckcraft_engine::cmd::xfrm_of(&st.doc, &st.selection, sh);
        let a = x.affine();
        let rot = if l.rotation != 0.0 {
            let c = l.inner.center().to_vec2();
            deckcraft_geom::Affine::translate(c) * deckcraft_geom::Affine::rotate(l.rotation.to_radians()) * deckcraft_geom::Affine::translate(-c)
        } else {
            deckcraft_geom::Affine::IDENTITY
        };
        let m = a * rot;
        let (s0, s1) = ts.ordered();
        for r in l.selection_rects(deckcraft_text::Pos { para: s0.0, ch: s0.1 }, deckcraft_text::Pos { para: s1.0, ch: s1.1 }) {
            let pts = [Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)].map(|q| xf.to_screen(m * q));
            painter.add(egui::Shape::convex_polygon(pts.to_vec(), t.text_selection, Stroke::NONE));
        }
        if !ts.is_range() {
            let blink = (ui.input(|i| i.time) * 1.6).fract() < 0.6;
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
            if blink && let Some((cx, top, bot)) = l.caret(deckcraft_text::Pos { para: ts.caret.0, ch: ts.caret.1 }) {
                let (p0, p1) = (xf.to_screen(m * Point::new(cx, top)), xf.to_screen(m * Point::new(cx, bot)));
                painter.line_segment([p0, p1], Stroke::new(1.5, t.caret));
            }
        }
    }
    // Table cell editing: outline the cell.
    if let Some(ts) = &editing
        && let (Some((r, c)), Some(sh)) = (ts.cell, st.shape(ts.shape))
        && let deckcraft_model::ShapeKind::Table(tb) = &sh.kind
    {
        let x = deckcraft_engine::cmd::xfrm_of(&st.doc, &st.selection, sh);
        let y0: f64 = tb.rows.iter().take(r).map(|rr| rr.height).sum();
        let x0: f64 = tb.cols.iter().take(c).sum();
        let (w, h) = (tb.cols.get(c).copied().unwrap_or(0.0), tb.rows.get(r).map(|rr| rr.height).unwrap_or(0.0));
        let a = x.affine();
        let pts = [Point::new(x0, y0), Point::new(x0 + w, y0), Point::new(x0 + w, y0 + h), Point::new(x0, y0 + h)].map(|q| xf.to_screen(a * q));
        poly(painter, pts, Stroke::new(2.0, t.accent));
    }
    // Smart guides, marquee, previews.
    for g in &app.session.tool.guides {
        let (a, b) =
            if g.vertical { (Point::new(g.pos, g.from), Point::new(g.pos, g.to)) } else { (Point::new(g.from, g.pos), Point::new(g.to, g.pos)) };
        painter.add(egui::Shape::dashed_line(&[xf.to_screen(a), xf.to_screen(b)], Stroke::new(1.0, t.guide), 5.0, 3.0));
    }
    if let Some(m) = app.session.tool.marquee {
        let r = Rect::from_two_pos(xf.to_screen(Point::new(m.x0, m.y0)), xf.to_screen(Point::new(m.x1, m.y1)));
        painter.rect(
            r,
            CornerRadius::ZERO,
            Color32::from_rgba_unmultiplied(80, 130, 230, 30),
            Stroke::new(1.0, Color32::from_rgb(80, 130, 230)),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(pv) = app.session.tool.preview {
        draw_preview(app, painter, xf, pv, t);
    }
    // Freeform being drawn.
    let path = &app.session.tool.path_preview;
    if path.len() > 1 {
        let pts: Vec<Point> = if app.session.tool.path_smooth && path.len() > 2 {
            let d = deckcraft_engine::cmd::shape::freeform_path(path, false, true);
            let mut v = vec![];
            let bp = deckcraft_render::parse_path(&d);
            deckcraft_geom::preset_flatten(&bp, &mut |el| match el {
                deckcraft_geom::PathEl::MoveTo(q) | deckcraft_geom::PathEl::LineTo(q) => v.push(q),
                _ => {}
            });
            v
        } else {
            path.clone()
        };
        painter.add(egui::Shape::line(pts.iter().map(|q| xf.to_screen(*q)).collect(), Stroke::new(1.5, t.accent)));
        if app.session.tool.path_closing
            && let Some(s0) = path.first()
        {
            painter.circle(xf.to_screen(*s0), 5.0, t.accent, Stroke::new(1.5, t.handle_fill));
        }
    }
    // Connection sites of the shape under a connector end, and the site it would glue to.
    for q in &app.session.tool.sites {
        let c = xf.to_screen(*q);
        painter.circle(c, 3.5, t.handle_fill, Stroke::new(1.0, t.handle_stroke));
    }
    if let Some(g) = app.session.tool.glue {
        painter.circle(xf.to_screen(g), 5.0, t.accent, Stroke::new(1.5, t.handle_fill));
    }
    if app.session.tool.ink_preview.len() > 1
        && let ToolKind::Ink { color, width, mode } = &app.session.tool.kind
    {
        let pts: Vec<Pos2> = app.session.tool.ink_preview.iter().map(|(x, y, _)| xf.to_screen(Point::new(*x, *y))).collect();
        let c = theme::to_color32(*color);
        let c = if mode == "highlighter" { c.gamma_multiply(0.45) } else { c };
        painter.add(egui::Shape::line(pts, Stroke::new((*width as f32 * xf.scale).max(1.0), c)));
    }
}

pub(super) fn rotate_handle(p: &egui::Painter, c: Pos2, t: &Tokens) {
    p.circle_filled(c, 6.0, t.handle_fill);
    p.add(egui::Shape::Path(egui::epaint::PathShape {
        points: (0..=20)
            .map(|i| {
                let a = -0.4 + i as f32 / 20.0 * 5.2;
                pos2(c.x + 4.0 * a.cos(), c.y + 4.0 * a.sin())
            })
            .collect(),
        closed: false,
        fill: Color32::TRANSPARENT,
        stroke: Stroke::new(1.3, t.handle_stroke).into(),
    }));
}

pub(super) fn draw_preview(app: &SlideApp, p: &egui::Painter, xf: Xf, x: Xfrm, t: &Tokens) {
    let preset = match &app.session.tool.kind {
        ToolKind::Shape { preset } => preset.clone(),
        _ => "rect".into(),
    };
    if deckcraft_geom::preset::is_line_like(&preset) {
        let a = x.affine();
        p.line_segment([xf.to_screen(a * Point::new(0.0, 0.0)), xf.to_screen(a * Point::new(x.w, x.h))], Stroke::new(1.5, t.accent));
        return;
    }
    let r = Rect::from_two_pos(xf.to_screen(Point::new(x.x, x.y)), xf.to_screen(Point::new(x.x + x.w, x.y + x.h)));
    if r.width() < 1.0 || r.height() < 1.0 {
        return;
    }
    crate::ribbon::paint_preset(p, r, &preset, t.accent.gamma_multiply(0.25), t.accent);
}
