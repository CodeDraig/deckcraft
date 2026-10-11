//! Shape, text and table hit testing in slide coordinates.

use super::Hit;
use super::transform::box_handles;
use crate::cmd::{self, xfrm_of};
use crate::{Session, TextSel};
use deckcraft_geom::{Point, Rect};
use deckcraft_model::{Shape, ShapeKind};

pub(super) fn hit_shape(st: &crate::DocState, sh: &Shape, p: Point, tol: f64) -> Option<Hit> {
    let x = xfrm_of(&st.doc, &st.selection, sh);
    let inv = x.affine().inverse();
    let local = inv * p;
    match &sh.kind {
        ShapeKind::Group { children, child } => {
            // A group is hit when any child is hit; the group is what gets selected.
            let ch = Rect::new(child.x, child.y, child.x + child.w, child.y + child.h);
            let m = deckcraft_geom::group_child_affine(&x, ch);
            let q = m.inverse() * p;
            for c in children.iter().rev() {
                if let Some(cx) = c.xfrm {
                    let cl = cx.affine().inverse() * q;
                    if cl.x >= -tol && cl.y >= -tol && cl.x <= cx.w + tol && cl.y <= cx.h + tol {
                        return Some(Hit::Shape { id: sh.id, text: false });
                    }
                }
            }
            None
        }
        ShapeKind::Ink { strokes } => {
            for s in strokes {
                if s.points.iter().any(|(px, py, _)| (Point::new(*px, *py) - p).hypot() <= tol + s.width) {
                    return Some(Hit::Shape { id: sh.id, text: false });
                }
            }
            None
        }
        _ => {
            if sh.is_line() {
                let geo = deckcraft_render::shape_geometry(sh, x.w, x.h);
                let lw = sh.line.as_ref().and_then(|l| l.width).unwrap_or(1.0);
                if deckcraft_geom::near_path(&geo.outline(), local, tol.max(lw)) {
                    return Some(Hit::Shape { id: sh.id, text: false });
                }
                return None;
            }
            let inside = local.x >= -tol && local.y >= -tol && local.x <= x.w + tol && local.y <= x.h + tol;
            if !inside {
                return None;
            }
            let has_text = matches!(sh.kind, ShapeKind::Shape) && (sh.text.as_ref().is_some_and(|t| !t.is_empty()) || sh.ph.is_some() || sh.text_box);
            // Empty interiors of unfilled, text-less shapes don't catch clicks.
            let filled = !matches!(sh.fill, Some(deckcraft_model::Fill::None)) || sh.style.is_some();
            let geo = deckcraft_render::shape_geometry(sh, x.w, x.h);
            let in_geo = {
                use deckcraft_geom::KurboShape;
                geo.outline().winding(local) != 0 || deckcraft_geom::near_path(&geo.outline(), local, tol)
            };
            if has_text
                || sh.ph.is_some()
                || matches!(
                    sh.kind,
                    ShapeKind::Picture { .. } | ShapeKind::Table(_) | ShapeKind::Chart(_) | ShapeKind::Media(_) | ShapeKind::Opaque { .. }
                )
                || (filled && in_geo)
                || deckcraft_geom::near_path(&geo.outline(), local, tol)
            {
                return Some(Hit::Shape { id: sh.id, text: has_text || matches!(sh.kind, ShapeKind::Shape) });
            }
            None
        }
    }
}

/// Text position under slide point `p` in shape `sh`.
pub fn text_pos(st: &crate::DocState, sh: &Shape, p: Point) -> (usize, usize) {
    let x = xfrm_of(&st.doc, &st.selection, sh);
    let local = x.affine().inverse() * p;
    let t = TextSel { shape: sh.id, ..Default::default() };
    match cmd::text::layout_for(st, &t) {
        Some(l) => {
            let h = l.hit(local);
            (h.para, h.ch)
        }
        None => (0, 0),
    }
}

/// Which table cell contains a table-local point.
pub(super) fn deckcraft_render_cell(t: &deckcraft_model::Table, p: Point) -> (usize, usize) {
    let mut y = 0.0;
    let mut row = t.rows.len().saturating_sub(1);
    for (i, r) in t.rows.iter().enumerate() {
        if p.y < y + r.height {
            row = i;
            break;
        }
        y += r.height;
    }
    let mut x = 0.0;
    let mut col = t.cols.len().saturating_sub(1);
    for (i, w) in t.cols.iter().enumerate() {
        if p.x < x + w {
            col = i;
            break;
        }
        x += w;
    }
    (row, col)
}

impl Session {
    /// What's under `p` on the current slide.
    pub fn hit_test(&self, p: Point, tol: f64) -> Option<Hit> {
        let st = self.active()?;
        // Handles of selected shapes first.
        if st.selection.text.is_none() || st.selection.shapes.len() == 1 {
            for id in st.selection.shapes.iter().rev() {
                let Some(sh) = st.shape(*id) else { continue };
                let x = xfrm_of(&st.doc, &st.selection, sh);
                if sh.is_line() {
                    let a = x.affine();
                    for (end, q) in [(0u8, a * Point::new(0.0, 0.0)), (1u8, a * Point::new(x.w, x.h))] {
                        if (q - p).hypot() <= tol * 1.6 {
                            return Some(Hit::LineEnd { id: *id, end });
                        }
                    }
                    continue;
                }
                let geo = deckcraft_render::shape_geometry(sh, x.w, x.h);
                for hd in &geo.handles {
                    if (x.affine() * hd.pos - p).hypot() <= tol * 1.6 {
                        return Some(Hit::Adjust { id: *id, index: hd.index });
                    }
                }
                let (pts, rot) = box_handles(&x);
                if (rot - p).hypot() <= tol * 1.8 && st.selection.text.is_none() {
                    return Some(Hit::Rotate { id: *id });
                }
                for (i, q) in pts.iter().enumerate() {
                    if (*q - p).hypot() <= tol * 1.6 {
                        return Some(Hit::Resize { id: *id, handle: i as u8 });
                    }
                }
            }
        }
        // Shapes, topmost first.
        for sh in st.shapes().iter().rev() {
            if sh.hidden || sh.locked {
                continue;
            }
            if let Some(h) = hit_shape(st, sh, p, tol) {
                return Some(h);
            }
        }
        None
    }
}
