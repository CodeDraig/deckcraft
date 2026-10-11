//! Smart guides and grid snapping for pointer transformations.

use super::{Guide, xfrm_of};
use crate::Session;
use deckcraft_geom::{Rect, Vec2, Xfrm};
use deckcraft_model::ShapeId;

impl Session {
    /// Smart guides: snap `r` edges/centres to the slide and other shapes. Returns the offset to
    /// apply and the guides to draw.
    pub(super) fn snap_rect(&self, r: Rect, exclude: &[ShapeId], tol: f64, enabled: bool) -> (Vec2, Vec<Guide>) {
        let Some(st) = self.active() else { return (Vec2::ZERO, vec![]) };
        if !enabled || !self.prefs.smart_guides {
            return (Vec2::ZERO, vec![]);
        }
        let size = st.doc.slide_size;
        let mut xs: Vec<(f64, Rect)> = vec![
            (0.0, Rect::new(0.0, 0.0, 0.0, size.height)),
            (size.width / 2.0, Rect::new(size.width / 2.0, 0.0, size.width / 2.0, size.height)),
            (size.width, Rect::new(size.width, 0.0, size.width, size.height)),
        ];
        let mut ys: Vec<(f64, Rect)> = vec![
            (0.0, Rect::new(0.0, 0.0, size.width, 0.0)),
            (size.height / 2.0, Rect::new(0.0, size.height / 2.0, size.width, size.height / 2.0)),
            (size.height, Rect::new(0.0, size.height, size.width, size.height)),
        ];
        for sh in st.shapes() {
            if exclude.contains(&sh.id) || sh.hidden {
                continue;
            }
            let b = xfrm_of(&st.doc, &st.selection, sh).bounds();
            for x in [b.x0, b.center().x, b.x1] {
                xs.push((x, b));
            }
            for y in [b.y0, b.center().y, b.y1] {
                ys.push((y, b));
            }
        }
        let mut best_x: Option<(f64, f64, Rect)> = None;
        for edge in [r.x0, r.center().x, r.x1] {
            for (x, b) in &xs {
                let d = x - edge;
                if d.abs() <= tol && best_x.is_none_or(|(bd, _, _)| d.abs() < bd.abs()) {
                    best_x = Some((d, *x, *b));
                }
            }
        }
        let mut best_y: Option<(f64, f64, Rect)> = None;
        for edge in [r.y0, r.center().y, r.y1] {
            for (y, b) in &ys {
                let d = y - edge;
                if d.abs() <= tol && best_y.is_none_or(|(bd, _, _)| d.abs() < bd.abs()) {
                    best_y = Some((d, *y, *b));
                }
            }
        }
        let mut off = Vec2::ZERO;
        let mut guides = vec![];
        if let Some((d, x, b)) = best_x {
            off.x = d;
            let rr = r + Vec2::new(d, 0.0);
            guides.push(Guide { vertical: true, pos: x, from: b.y0.min(rr.y0), to: b.y1.max(rr.y1) });
        } else if self.prefs.snap_to_grid {
            off.x = deckcraft_geom::snap(r.x0, self.prefs.grid_spacing) - r.x0;
        }
        if let Some((d, y, b)) = best_y {
            off.y = d;
            let rr = r + Vec2::new(0.0, d);
            guides.push(Guide { vertical: false, pos: y, from: b.x0.min(rr.x0), to: b.x1.max(rr.x1) });
        } else if self.prefs.snap_to_grid {
            off.y = deckcraft_geom::snap(r.y0, self.prefs.grid_spacing) - r.y0;
        }
        (off, guides)
    }

    pub(super) fn snap_resize(&mut self, x: Xfrm, _orig: &Xfrm, handle: u8, id: ShapeId, tol: f64) -> Xfrm {
        if x.rot != 0.0 {
            self.tool.guides.clear();
            return x;
        }
        let r = x.rect();
        let (off, guides) = self.snap_rect(r, &[id], tol * 1.5, true);
        let mut nx = x;
        // Only the dragged edges move.
        let moves_left = matches!(handle, 0 | 6 | 7);
        let moves_right = matches!(handle, 2..=4);
        let moves_top = matches!(handle, 0..=2);
        let moves_bottom = matches!(handle, 4..=6);
        if moves_left {
            nx.x += off.x;
            nx.w -= off.x;
        } else if moves_right {
            nx.w += off.x;
        }
        if moves_top {
            nx.y += off.y;
            nx.h -= off.y;
        } else if moves_bottom {
            nx.h += off.y;
        }
        nx.w = nx.w.max(0.0);
        nx.h = nx.h.max(0.0);
        self.tool.guides = guides;
        nx
    }
}
