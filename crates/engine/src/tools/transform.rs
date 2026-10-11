//! Pointer handle geometry and constrained shape transforms.

use deckcraft_geom::{Affine, Point, Rect, Vec2, Xfrm};

/// Handle positions in slide coordinates for a box (8 resize + rotate).
pub fn box_handles(x: &Xfrm) -> ([Point; 8], Point) {
    let a = x.affine();
    let (w, h) = (x.w, x.h);
    let local = [(0.0, 0.0), (w / 2.0, 0.0), (w, 0.0), (w, h / 2.0), (w, h), (w / 2.0, h), (0.0, h), (0.0, h / 2.0)];
    let pts = local.map(|(px, py)| a * Point::new(px, py));
    // The rotation handle sits above the top edge (in the shape's rotated frame).
    let up = if x.flip_v { 1.0 } else { -1.0 };
    let rot =
        Affine::translate(Vec2::new(x.x + w / 2.0, x.y + h / 2.0)) * Affine::rotate(x.rot.to_radians()) * Point::new(0.0, up * (h / 2.0 + 20.0));
    (pts, rot)
}

pub(super) fn constrain_45(a: Point, b: Point) -> Point {
    let d = b - a;
    let ang = d.y.atan2(d.x);
    let snapped = (ang / std::f64::consts::FRAC_PI_4).round() * std::f64::consts::FRAC_PI_4;
    let len = d.hypot();
    a + Vec2::new(snapped.cos(), snapped.sin()) * len
}

/// A line-like shape's box for endpoints `a` → `b` (flips encode direction).
pub fn line_xfrm(a: Point, b: Point, _rot: f64) -> Xfrm {
    let r = Rect::from_points(a, b);
    Xfrm { x: r.x0, y: r.y0, w: r.width(), h: r.height(), rot: 0.0, flip_h: b.x < a.x, flip_v: b.y < a.y }
}

/// New box when dragging resize handle `h` by `d` (slide space), keeping the shape's rotation.
pub fn resize_box(o: &Xfrm, h: u8, d: Vec2, keep_aspect: bool, from_center: bool) -> Xfrm {
    // Work in the shape's rotated frame.
    let rot = Affine::rotate(-o.rot.to_radians());
    let ld = rot * Point::new(d.x, d.y);
    let (mut l, mut t, mut r, mut b) = (0.0, 0.0, o.w, o.h);
    let (sx, sy) = (if o.flip_h { -1.0 } else { 1.0 }, if o.flip_v { -1.0 } else { 1.0 });
    let (dx, dy) = (ld.x * sx, ld.y * sy);
    match h {
        0 => {
            l += dx;
            t += dy;
        }
        1 => t += dy,
        2 => {
            r += dx;
            t += dy;
        }
        3 => r += dx,
        4 => {
            r += dx;
            b += dy;
        }
        5 => b += dy,
        6 => {
            l += dx;
            b += dy;
        }
        _ => l += dx,
    }
    if from_center {
        // Symmetric growth about the centre.
        let gw = match h {
            0 | 6 | 7 => -dx,
            2..=4 => dx,
            _ => 0.0,
        };
        let gh = match h {
            0..=2 => -dy,
            4..=6 => dy,
            _ => 0.0,
        };
        l = -gw;
        r = o.w + gw;
        t = -gh;
        b = o.h + gh;
    }
    let mut w = r - l;
    let mut hh = b - t;
    if keep_aspect && o.w > 1e-9 && o.h > 1e-9 && !matches!(h, 1 | 3 | 5 | 7) {
        let ar = o.w / o.h;
        if (w / ar).abs() > hh.abs() {
            hh = w.abs() / ar * hh.signum().max(if hh == 0.0 { 1.0 } else { hh.signum() });
        } else {
            w = hh.abs() * ar * w.signum().max(if w == 0.0 { 1.0 } else { w.signum() });
        }
        if matches!(h, 0 | 6) {
            l = r - w;
        } else {
            r = l + w;
        }
        if matches!(h, 0 | 2) {
            t = b - hh;
        } else {
            b = t + hh;
        }
        if from_center {
            let (cx, cy) = (o.w / 2.0, o.h / 2.0);
            l = cx - w / 2.0;
            r = cx + w / 2.0;
            t = cy - hh / 2.0;
            b = cy + hh / 2.0;
        }
    }
    // Dragging past the opposite edge flips.
    let (mut fh, mut fv) = (o.flip_h, o.flip_v);
    if r < l {
        std::mem::swap(&mut l, &mut r);
        fh = !fh;
    }
    if b < t {
        std::mem::swap(&mut t, &mut b);
        fv = !fv;
    }
    let (nw, nh) = (r - l, b - t);
    // New centre in slide space: old centre + rotated offset of the local centre change.
    let local_c = Point::new((l + r) / 2.0 * sx + if sx < 0.0 { o.w } else { 0.0 }, (t + b) / 2.0 * sy + if sy < 0.0 { o.h } else { 0.0 });
    let c = o.affine() * Point::new(if o.flip_h { o.w - local_c.x } else { local_c.x }, if o.flip_v { o.h - local_c.y } else { local_c.y });
    Xfrm { x: c.x - nw / 2.0, y: c.y - nh / 2.0, w: nw, h: nh, rot: o.rot, flip_h: fh, flip_v: fv }
}
