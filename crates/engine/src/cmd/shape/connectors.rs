//! Construct freeform shapes and connect shape sites.

use super::super::*;
use super::freeform_path;
use crate::{Result, Session};
use deckcraft_model::Geom;
use serde_json::{Value, json};

pub(super) fn shape_id(p: &Value, key: &str) -> Option<deckcraft_model::ShapeId> {
    p.get(key).and_then(Value::as_u64).map(|v| deckcraft_model::ShapeId(v as u32))
}

pub(super) fn sites(s: &mut Session, p: &Value) -> Result<Value> {
    let id = shape_id(p, "id").ok_or_else(|| bad("shape.sites", "missing `id`"))?;
    let st = s.doc()?;
    let sh = st.shape(id).ok_or_else(|| bad("shape.sites", "no such shape"))?;
    Ok(json!(crate::connect::sites(&st.doc, &st.selection, sh).iter().map(|q| [q.x, q.y]).collect::<Vec<_>>()))
}

pub(super) fn connect(s: &mut Session, p: &Value) -> Result<Value> {
    let (Some(from), Some(to)) = (shape_id(p, "from"), shape_id(p, "to")) else { return Err(bad("shape.connect", "missing `from` / `to`")) };
    let st = s.doc()?;
    let site_list = |id| st.shape(id).map(|sh| crate::connect::sites(&st.doc, &st.selection, sh)).unwrap_or_default();
    let (a, b) = (site_list(from), site_list(to));
    if a.is_empty() || b.is_empty() {
        return Err(bad("shape.connect", "both shapes need connection sites (lines and groups have none)"));
    }
    let pick = |v: &[deckcraft_geom::Point], key: &str| usize_param(p, key).filter(|i| *i < v.len());
    let (ia, ib) = match (pick(&a, "fromSite"), pick(&b, "toSite")) {
        (Some(i), Some(j)) => (i, j),
        (fi, fj) => {
            let mut best = (f64::MAX, 0, 0);
            for (i, pa) in a.iter().enumerate().filter(|(i, _)| fi.is_none_or(|f| f == *i)) {
                for (j, pb) in b.iter().enumerate().filter(|(j, _)| fj.is_none_or(|f| f == *j)) {
                    let d = (*pa - *pb).hypot();
                    if d < best.0 {
                        best = (d, i, j);
                    }
                }
            }
            (best.1, best.2)
        }
    };
    let (p0, p1) = (a[ia], b[ib]);
    let id = match shape_id(p, "id") {
        Some(id) => id,
        None => {
            let preset = str_param(p, "preset").unwrap_or("straightConnector1");
            let r = s.execute("shape.insert", &json!({"preset": preset, "rect": [p0.x, p0.y, 1.0, 1.0]}))?;
            deckcraft_model::ShapeId(r.get("id").and_then(Value::as_u64).ok_or_else(|| bad("shape.connect", "insert failed"))? as u32)
        }
    };
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad("shape.connect", "no slide"))?;
        let sh = deckcraft_model::find_shape_mut(list, id).ok_or_else(|| bad("shape.connect", "no such line"))?;
        if !sh.is_line() {
            return Err(bad("shape.connect", "`id` is not a line or connector"));
        }
        sh.xfrm = Some(crate::tools::line_xfrm(p0, p1, 0.0));
        sh.kind = deckcraft_model::ShapeKind::Connector { start: Some((from, ia as u32)), end: Some((to, ib as u32)) };
        Ok(())
    })?;
    Ok(json!({"id": id.0, "fromSite": ia, "toSite": ib}))
}

pub(super) fn freeform(s: &mut Session, p: &Value) -> Result<Value> {
    let pts: Vec<deckcraft_geom::Point> = p
        .get("points")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| Some(deckcraft_geom::Point::new(v.get(0)?.as_f64()?, v.get(1)?.as_f64()?)))
                .filter(|q| q.x.is_finite() && q.y.is_finite())
                .take(20_000)
                .collect()
        })
        .unwrap_or_default();
    if pts.len() < 2 {
        return Err(bad("shape.freeform", "need at least two points"));
    }
    let closed = bool_or(p, "closed", false) && pts.len() > 2;
    let smooth = bool_or(p, "smooth", false);
    let mut bb = deckcraft_geom::Rect::from_points(pts[0], pts[0]);
    for q in &pts {
        bb = bb.union_pt(*q);
    }
    let (w, h) = (bb.width().max(1.0), bb.height().max(1.0));
    let local: Vec<deckcraft_geom::Point> = pts.iter().map(|q| deckcraft_geom::Point::new(q.x - bb.x0, q.y - bb.y0)).collect();
    let d = freeform_path(&local, closed, smooth);
    // Closed shapes take the default shape style, open paths the line style.
    let r = s.execute("shape.insert", &json!({"preset": if closed { "rect" } else { "line" }, "rect": [bb.x0, bb.y0, w, h]}))?;
    let id = deckcraft_model::ShapeId(r.get("id").and_then(Value::as_u64).ok_or_else(|| bad("shape.freeform", "insert failed"))? as u32);
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad("shape.freeform", "no slide"))?;
        let sh = deckcraft_model::find_shape_mut(list, id).ok_or_else(|| bad("shape.freeform", "insert failed"))?;
        sh.geom = Geom::Custom {
            paths: vec![deckcraft_model::CustomPath {
                w,
                h,
                d: d.clone(),
                fill: if closed { deckcraft_geom::preset::FillMode::Norm } else { deckcraft_geom::preset::FillMode::None },
                stroke: true,
            }],
        };
        sh.xfrm = Some(deckcraft_geom::Xfrm::new(bb.x0, bb.y0, w, h));
        sh.kind = deckcraft_model::ShapeKind::Shape;
        sh.name = format!("Freeform {}", id.0);
        Ok(())
    })?;
    Ok(json!({"id": id.0}))
}
