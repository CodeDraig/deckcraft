//! Pointer tools. Gestures arrive in slide coordinates (points) through [`Session::pointer`] from
//! the UI, the control channel or MCP alike; each finished gesture is one undo step.

mod snap;

mod hit_test;
use hit_test::deckcraft_render_cell;
pub use hit_test::text_pos;

mod transform;
use transform::constrain_45;
pub use transform::{box_handles, line_xfrm, resize_box};

use std::sync::Arc;

use deckcraft_geom::{Affine, Point, Rect, Vec2, Xfrm};
use deckcraft_model::{Shape, ShapeId, ShapeKind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cmd::{self, xfrm_of};
use crate::{HistoryEntry, Result, Session};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PointerKind {
    #[default]
    Down,
    Drag,
    Up,
    Move,
    DoubleClick,
    TripleClick,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub cmd: bool,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PointerEvent {
    pub kind: PointerKind,
    pub x: f64,
    pub y: f64,
    pub mods: Mods,
    /// Hit tolerance in points (≈ 4 screen px at the current zoom).
    pub tol: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ToolKind {
    #[default]
    Select,
    /// Draw a preset shape.
    Shape {
        preset: String,
    },
    TextBox,
    /// Ink: pen (`pen`, `highlighter`) or `eraser`.
    Ink {
        mode: String,
        color: deckcraft_color::Rgba,
        width: f64,
    },
}

/// What a point hits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Hit {
    /// Resize handle 0..8: NW, N, NE, E, SE, S, SW, W.
    Resize {
        id: ShapeId,
        handle: u8,
    },
    Rotate {
        id: ShapeId,
    },
    Adjust {
        id: ShapeId,
        index: usize,
    },
    /// Line end 0/1 of a line-like shape.
    LineEnd {
        id: ShapeId,
        end: u8,
    },
    Shape {
        id: ShapeId,
        text: bool,
    },
}

#[derive(Clone, Debug)]
enum Drag {
    Move {
        ids: Vec<ShapeId>,
        start: Point,
        origs: Vec<(ShapeId, Xfrm)>,
        moved: bool,
        duplicate: bool,
        enter_text: Option<(ShapeId, Point)>,
    },
    Resize {
        id: ShapeId,
        handle: u8,
        orig: Xfrm,
        start: Point,
    },
    LineEnd {
        id: ShapeId,
        end: u8,
        orig: Xfrm,
    },
    Rotate {
        ids: Vec<ShapeId>,
        center: Point,
        start: f64,
        origs: Vec<(ShapeId, Xfrm)>,
    },
    Adjust {
        id: ShapeId,
        index: usize,
        orig: Xfrm,
    },
    Marquee {
        start: Point,
        add: bool,
    },
    Create {
        start: Point,
    },
    TextSelect {
        shape: ShapeId,
    },
    Ink {
        points: Vec<(f64, f64, f32)>,
    },
    /// Freeform drawing (`freeform` polygon, `curve`, `scribble`): placed points; `pressed` while
    /// the button is down (dragging adds scribbled points).
    Freeform {
        mode: String,
        points: Vec<Point>,
        pressed: bool,
    },
}

/// Shape-gallery names that draw a freeform path instead of a preset.
pub const FREEFORM_TOOLS: [(&str, &str); 3] = [("curve", "Curve"), ("freeform", "Freeform: Shape"), ("scribble", "Freeform: Scribble")];

pub fn is_freeform_tool(name: &str) -> bool {
    FREEFORM_TOOLS.iter().any(|f| f.0 == name)
}

/// A snapping guide line to draw (slide coordinates): vertical (x) or horizontal (y).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Guide {
    pub vertical: bool,
    pub pos: f64,
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, Default)]
pub struct ToolState {
    pub kind: ToolKind,
    /// Keep the tool after one use (double-click on a shape in the gallery: lock drawing mode).
    pub sticky: bool,
    drag: Option<Drag>,
    pub marquee: Option<Rect>,
    pub preview: Option<Xfrm>,
    pub guides: Vec<Guide>,
    pub hover: Option<Hit>,
    pub ink_preview: Vec<(f64, f64, f32)>,
    /// Connection sites to show (slide coordinates) while drawing or dragging a line end, and the
    /// site the end would glue to.
    pub sites: Vec<Point>,
    pub glue: Option<Point>,
    /// Freeform being drawn: points so far (plus the rubber-band point), smoothed, and whether
    /// the pointer is over the start (closing the shape).
    pub path_preview: Vec<Point>,
    pub path_smooth: bool,
    pub path_closing: bool,
}

impl ToolState {
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// A freeform path is being drawn (Enter/Escape/double-click finish it).
    pub fn drawing_freeform(&self) -> bool {
        matches!(self.drag, Some(Drag::Freeform { .. }))
    }
}

impl Session {
    pub fn set_tool(&mut self, kind: ToolKind) {
        self.tool.kind = kind;
        self.tool.drag = None;
        self.tool.preview = None;
        self.tool.marquee = None;
    }

    /// Feed a pointer event to the active tool. Returns what happened (for agents).
    pub fn pointer(&mut self, ev: PointerEvent) -> Result<Value> {
        let tol = if ev.tol > 0.0 { ev.tol } else { 4.0 };
        let p = Point::new(ev.x, ev.y);
        if !(p.x.is_finite() && p.y.is_finite()) {
            return Ok(Value::Null);
        }
        match ev.kind {
            PointerKind::Move => {
                if let Some(Drag::Freeform { points, mode, .. }) = &self.tool.drag {
                    let closing = points.len() > 2 && points.first().is_some_and(|s| (*s - p).hypot() <= tol * 2.0);
                    let mut v = points.clone();
                    v.push(p);
                    self.tool.path_preview = v;
                    self.tool.path_smooth = mode == "curve";
                    self.tool.path_closing = closing;
                    return Ok(Value::Null);
                }
                self.tool.hover = self.hit_test(p, tol);
                let line_tool = matches!(&self.tool.kind, ToolKind::Shape { preset } if deckcraft_geom::preset::is_line_like(preset));
                if line_tool {
                    self.update_sites(p, tol, None);
                } else {
                    self.tool.sites.clear();
                    self.tool.glue = None;
                }
                Ok(Value::Null)
            }
            PointerKind::Down => self.down(p, ev.mods, tol),
            PointerKind::Drag => self.drag(p, ev.mods, tol),
            PointerKind::Up => self.up(p, ev.mods),
            PointerKind::DoubleClick => self.double_click(p, tol),
            PointerKind::TripleClick => {
                if self.active().is_some_and(|d| d.selection.text.is_some()) {
                    self.execute("text.selectParagraph", &json!({}))
                } else {
                    Ok(Value::Null)
                }
            }
        }
    }

    fn begin(&mut self, label: &str) {
        if let Some(st) = self.active_mut()
            && st.interaction.is_none()
        {
            st.interaction = Some(crate::Interaction { label: label.into(), doc: st.doc.clone(), selection: st.selection.clone() });
        }
    }

    fn commit(&mut self) {
        if let Some(st) = self.active_mut()
            && let Some(i) = st.interaction.take()
            && !Arc::ptr_eq(&i.doc, &st.doc)
        {
            crate::push_undo(st, HistoryEntry { label: i.label, doc: i.doc, selection: i.selection });
        }
    }

    fn down(&mut self, p: Point, mods: Mods, tol: f64) -> Result<Value> {
        if let Some(Drag::Freeform { mode, mut points, .. }) = self.tool.drag.clone() {
            // Next click of a freeform: close on the start point, else add a vertex.
            if points.len() > 2 && points.first().is_some_and(|s| (*s - p).hypot() <= tol * 2.0) {
                self.tool.drag = Some(Drag::Freeform { mode, points, pressed: false });
                return self.finish_freeform(true);
            }
            points.push(p);
            self.tool.path_preview = points.clone();
            self.tool.drag = Some(Drag::Freeform { mode, points, pressed: true });
            return Ok(Value::Null);
        }
        match self.tool.kind.clone() {
            ToolKind::Shape { preset } if is_freeform_tool(&preset) => {
                self.tool.drag = Some(Drag::Freeform { mode: preset.clone(), points: vec![p], pressed: true });
                self.tool.path_preview = vec![p];
                self.tool.path_smooth = preset == "curve";
                self.tool.path_closing = false;
                return Ok(Value::Null);
            }
            ToolKind::Shape { .. } | ToolKind::TextBox => {
                self.tool.drag = Some(Drag::Create { start: p });
                self.tool.preview = Some(Xfrm::new(p.x, p.y, 0.0, 0.0));
                return Ok(Value::Null);
            }
            ToolKind::Ink { mode, .. } => {
                if mode == "eraser" {
                    return self.erase_at(p, tol);
                }
                self.tool.drag = Some(Drag::Ink { points: vec![(p.x, p.y, 0.5)] });
                self.tool.ink_preview = vec![(p.x, p.y, 0.5)];
                return Ok(Value::Null);
            }
            ToolKind::Select => {}
        }
        let st = self.doc()?;
        // Clicking inside the text being edited moves the caret / starts a text selection.
        if let Some(t) = st.selection.text.clone()
            && !t.notes
            && t.cell.is_none()
            && let Some(sh) = st.shape(t.shape)
        {
            let x = xfrm_of(&st.doc, &st.selection, sh);
            if x.bounds().inflate(tol, tol).contains(p)
                && !matches!(self.hit_test(p, tol), Some(Hit::Resize { .. } | Hit::Rotate { .. } | Hit::Adjust { .. }))
            {
                let pos = text_pos(st, sh, p);
                let shift = mods.shift;
                self.select(|_, sel| {
                    if let Some(tt) = sel.text.as_mut() {
                        tt.caret = pos;
                        if !shift {
                            tt.anchor = pos;
                        }
                    }
                })?;
                self.tool.drag = Some(Drag::TextSelect { shape: t.shape });
                return Ok(json!({"caret": [pos.0, pos.1]}));
            }
        }
        let hit = self.hit_test(p, tol);
        let st = self.doc()?;
        match hit {
            Some(Hit::Resize { id, handle }) => {
                let orig = st.shape(id).map(|s| xfrm_of(&st.doc, &st.selection, s)).unwrap_or_default();
                self.begin("Size");
                self.tool.drag = Some(Drag::Resize { id, handle, orig, start: p });
            }
            Some(Hit::LineEnd { id, end }) => {
                let orig = st.shape(id).map(|s| xfrm_of(&st.doc, &st.selection, s)).unwrap_or_default();
                self.begin("Size");
                self.tool.drag = Some(Drag::LineEnd { id, end, orig });
            }
            Some(Hit::Rotate { id }) => {
                let ids: Vec<ShapeId> = if st.selection.shapes.contains(&id) { st.selection.shapes.clone() } else { vec![id] };
                let origs: Vec<(ShapeId, Xfrm)> = ids.iter().filter_map(|i| st.shape(*i).map(|s| (*i, xfrm_of(&st.doc, &st.selection, s)))).collect();
                let center = st.shape(id).map(|s| xfrm_of(&st.doc, &st.selection, s).center()).unwrap_or(p);
                let start = (p.y - center.y).atan2(p.x - center.x);
                self.begin("Rotate");
                self.tool.drag = Some(Drag::Rotate { ids, center, start, origs });
            }
            Some(Hit::Adjust { id, index }) => {
                let orig = st.shape(id).map(|s| xfrm_of(&st.doc, &st.selection, s)).unwrap_or_default();
                self.begin("Adjust Shape");
                self.tool.drag = Some(Drag::Adjust { id, index, orig });
            }
            Some(Hit::Shape { id, text }) => {
                let selected = st.selection.shapes.contains(&id);
                let toggle = mods.shift || mods.cmd;
                let ids: Vec<ShapeId> = if toggle {
                    let mut v = st.selection.shapes.clone();
                    if selected {
                        v.retain(|x| *x != id);
                    } else {
                        v.push(id);
                    }
                    v
                } else if selected {
                    st.selection.shapes.clone()
                } else {
                    vec![id]
                };
                let origs: Vec<(ShapeId, Xfrm)> = ids.iter().filter_map(|i| st.shape(*i).map(|s| (*i, xfrm_of(&st.doc, &st.selection, s)))).collect();
                // A click (no drag) on text puts the caret there; unselected text boxes and
                // placeholders take the caret straight away like PowerPoint.
                let enter_text = (text && !toggle && (selected || is_text_first(st, id))).then_some((id, p));
                let ids2 = ids.clone();
                self.select(|_, sel| {
                    sel.shapes = ids2;
                    sel.text = None;
                    sel.cells = None;
                })?;
                self.tool.drag = Some(Drag::Move { ids, start: p, origs, moved: false, duplicate: mods.alt, enter_text });
                // Format Painter applies on click.
                if self.painter.is_some() && !toggle {
                    let _ = self.execute("format.painterApply", &json!({"ids": [id]}));
                }
            }
            None => {
                let add = mods.shift || mods.cmd;
                self.select(|_, sel| {
                    sel.text = None;
                    sel.cells = None;
                    if !add {
                        sel.shapes.clear();
                    }
                })?;
                self.tool.drag = Some(Drag::Marquee { start: p, add });
                self.tool.marquee = Some(Rect::from_points(p, p));
            }
        }
        Ok(json!({"hit": serde_json::to_value(&hit).unwrap_or_default()}))
    }

    fn drag(&mut self, p: Point, mods: Mods, tol: f64) -> Result<Value> {
        let Some(drag) = self.tool.drag.clone() else { return Ok(Value::Null) };
        match drag {
            Drag::Move { ids, start, origs, moved, duplicate, enter_text } => {
                let mut d = p - start;
                if !moved && d.hypot() < tol.max(1.0) {
                    return Ok(Value::Null);
                }
                if !moved {
                    self.begin(if duplicate { "Duplicate" } else { "Move" });
                    if duplicate {
                        // Alt-drag: drag a copy, leave the original.
                        self.execute_dup_in_place(&ids)?;
                    }
                }
                let ids_now = if duplicate { self.doc()?.selection.shapes.clone() } else { ids.clone() };
                if mods.shift {
                    if d.x.abs() > d.y.abs() {
                        d.y = 0.0;
                    } else {
                        d.x = 0.0;
                    }
                }
                // Smart guides on the moved group's bounds.
                let bounds = origs.iter().map(|(_, x)| x.bounds()).reduce(|a, b| a.union(b)).unwrap_or_default();
                let moved_b = bounds + d;
                let (snap, guides) = self.snap_rect(moved_b, &ids_now, tol * 1.5, !mods.cmd);
                d += snap;
                self.tool.guides = guides;
                let origs2: Vec<(ShapeId, Xfrm)> =
                    if duplicate { ids_now.iter().copied().zip(origs.iter().map(|o| o.1)).collect() } else { origs.clone() };
                self.edit(|doc, sel| {
                    let list = crate::shapes_mut(doc, sel).ok_or_else(|| cmd::bad("move", "no slide"))?;
                    for (id, o) in &origs2 {
                        if let Some(sh) = deckcraft_model::find_shape_mut(list, *id) {
                            sh.xfrm = Some(Xfrm { x: o.x + d.x, y: o.y + d.y, ..*o });
                        }
                    }
                    // A connector dragged away from shapes that stay put comes unglued at those ends.
                    let moved: Vec<ShapeId> = origs2.iter().map(|o| o.0).collect();
                    for sh in list.iter_mut().filter(|s| moved.contains(&s.id)) {
                        if let ShapeKind::Connector { start, end } = &mut sh.kind {
                            for c in [start, end] {
                                if c.is_some_and(|(to, _)| !moved.contains(&to)) {
                                    *c = None;
                                }
                            }
                        }
                    }
                    Ok(())
                })?;
                self.tool.drag =
                    Some(Drag::Move { ids: ids_now, start, origs: origs2, moved: true, duplicate: false, enter_text: enter_text.filter(|_| false) });
            }
            Drag::Resize { id, handle, orig, start } => {
                let nx = resize_box(&orig, handle, p - start, mods.shift || self.is_picture(id), mods.alt);
                let nx = self.snap_resize(nx, &orig, handle, id, tol);
                self.set_xfrm(id, nx)?;
            }
            Drag::LineEnd { id, end, orig } => {
                let a = orig.affine();
                let (p0, p1) = (a * Point::new(0.0, 0.0), a * Point::new(orig.w, orig.h));
                let mut q = p;
                let other = if end == 0 { p1 } else { p0 };
                if mods.shift {
                    q = constrain_45(other, q);
                }
                let glue = self.update_sites(q, tol, Some(id));
                if let Some((_, _, at)) = glue {
                    q = at;
                }
                let (s0, s1) = if end == 0 { (q, p1) } else { (p0, q) };
                self.set_xfrm(id, line_xfrm(s0, s1, orig.rot))?;
                self.edit(|doc, sel| {
                    if let Some(list) = crate::shapes_mut(doc, sel) {
                        crate::connect::set_glue(list, id, end, glue.map(|g| (g.0, g.1)));
                    }
                    Ok(())
                })?;
            }
            Drag::Rotate { ids, center, start, origs } => {
                let ang = (p.y - center.y).atan2(p.x - center.x);
                let mut delta = (ang - start).to_degrees();
                for (id, o) in &origs {
                    let mut r = o.rot + delta;
                    if mods.shift {
                        r = (r / 15.0).round() * 15.0;
                        delta = r - o.rot;
                    }
                    let mut nx = *o;
                    nx.rot = ((r % 360.0) + 360.0) % 360.0;
                    if ids.len() > 1 {
                        // Rotate positions around the common centre too.
                        let c = o.center();
                        let rc = Affine::translate(center.to_vec2()) * Affine::rotate(delta.to_radians()) * Affine::translate(-center.to_vec2()) * c;
                        nx.x = rc.x - o.w / 2.0;
                        nx.y = rc.y - o.h / 2.0;
                    }
                    self.set_xfrm(*id, nx)?;
                }
            }
            Drag::Adjust { id, index, orig } => {
                let st = self.doc()?;
                let Some(sh) = st.shape(id).cloned() else { return Ok(Value::Null) };
                let local = orig.affine().inverse() * p;
                let geo = deckcraft_render::shape_geometry(&sh, orig.w, orig.h);
                let Some(h) = geo.handles.iter().find(|h| h.index == index) else { return Ok(Value::Null) };
                let free2d = matches!(sh.geom.preset_name(), Some(n) if n.contains("Callout") && !n.contains("Arrow"));
                let vals: Vec<(usize, f64)> = if free2d {
                    let dx = (local.x - orig.w / 2.0) / orig.w.max(1e-6) * 1e5;
                    let dy = (local.y - orig.h / 2.0) / orig.h.max(1e-6) * 1e5;
                    vec![(0, dx.round()), (1, dy.round())]
                } else {
                    vec![(index, h.value_at(local))]
                };
                self.edit(|doc, sel| {
                    let list = crate::shapes_mut(doc, sel).ok_or_else(|| cmd::bad("adjust", "no slide"))?;
                    if let Some(sh) = deckcraft_model::find_shape_mut(list, id)
                        && let deckcraft_model::Geom::Preset { name, adj } = &mut sh.geom
                    {
                        let defaults = deckcraft_geom::preset::info(name).map(|x| x.defaults).unwrap_or(&[]);
                        for (i, v) in &vals {
                            while adj.len() <= *i {
                                let k = adj.len();
                                adj.push(defaults.get(k).copied().unwrap_or(0.0));
                            }
                            if let Some(slot) = adj.get_mut(*i) {
                                *slot = *v;
                            }
                        }
                    }
                    Ok(())
                })?;
            }
            Drag::Marquee { start, .. } => {
                self.tool.marquee = Some(Rect::from_points(start, p));
            }
            Drag::Create { start } => {
                let line = matches!(&self.tool.kind, ToolKind::Shape { preset } if deckcraft_geom::preset::is_line_like(preset));
                let x = if line {
                    let mut q = if mods.shift { constrain_45(start, p) } else { p };
                    if let Some((_, _, at)) = self.update_sites(q, tol, None) {
                        q = at;
                    }
                    let s0 = self.glue_at(start, tol).map(|g| g.2).unwrap_or(start);
                    line_xfrm(s0, q, 0.0)
                } else {
                    let mut r = Rect::from_points(start, p);
                    if mods.shift {
                        let s = r.width().max(r.height());
                        let sx = if p.x < start.x { -s } else { s };
                        let sy = if p.y < start.y { -s } else { s };
                        r = Rect::from_points(start, start + Vec2::new(sx, sy));
                    }
                    if mods.alt {
                        let d = p - start;
                        r = Rect::new(start.x - d.x.abs(), start.y - d.y.abs(), start.x + d.x.abs(), start.y + d.y.abs());
                    }
                    Xfrm::new(r.x0, r.y0, r.width(), r.height())
                };
                self.tool.preview = Some(x);
            }
            Drag::TextSelect { shape } => {
                let st = self.doc()?;
                if let Some(sh) = st.shape(shape) {
                    let pos = text_pos(st, sh, p);
                    self.select(|_, sel| {
                        if let Some(t) = sel.text.as_mut() {
                            t.caret = pos;
                        }
                    })?;
                }
            }
            Drag::Freeform { mode, mut points, pressed } => {
                // Dragging scribbles (except for curves, which only take clicked points).
                if pressed && mode != "curve" && points.last().is_none_or(|l| (*l - p).hypot() >= 1.5) {
                    points.push(p);
                }
                let mut v = points.clone();
                if mode == "curve" {
                    v.push(p);
                }
                self.tool.path_preview = v;
                self.tool.drag = Some(Drag::Freeform { mode, points, pressed });
            }
            Drag::Ink { mut points } => {
                points.push((p.x, p.y, 0.5));
                self.tool.ink_preview = points.clone();
                self.tool.drag = Some(Drag::Ink { points });
            }
        }
        Ok(Value::Null)
    }

    fn up(&mut self, p: Point, mods: Mods) -> Result<Value> {
        let drag = self.tool.drag.take();
        self.tool.guides.clear();
        self.tool.sites.clear();
        self.tool.glue = None;
        let mut out = Value::Null;
        match drag {
            Some(Drag::Move { moved: false, enter_text: Some((id, at)), .. }) => {
                // Plain click on text: caret at the click.
                let st = self.doc()?;
                if let Some(sh) = st.shape(id) {
                    let pos = text_pos(st, sh, at);
                    self.execute("text.edit", &json!({"id": id, "at": [pos.0, pos.1]}))?;
                    out = json!({"editing": id});
                }
            }
            Some(Drag::Marquee { start, add }) => {
                let r = Rect::from_points(start, p);
                self.tool.marquee = None;
                if r.width() > 1.0 || r.height() > 1.0 {
                    let st = self.doc()?;
                    let ids: Vec<ShapeId> = st
                        .shapes()
                        .iter()
                        .filter(|s| !s.hidden && !s.locked)
                        .filter(|s| contains_rect(r, xfrm_of(&st.doc, &st.selection, s).bounds()))
                        .map(|s| s.id)
                        .collect();
                    self.select(|_, sel| {
                        if add {
                            for i in &ids {
                                if !sel.shapes.contains(i) {
                                    sel.shapes.push(*i);
                                }
                            }
                        } else {
                            sel.shapes = ids.clone();
                        }
                    })?;
                    out = json!({"selected": ids});
                }
            }
            Some(Drag::Create { start }) => {
                let kind = self.tool.kind.clone();
                let preview = self.tool.preview.take();
                let dragged = (p - start).hypot() > 2.0;
                let res = match &kind {
                    ToolKind::Shape { preset } => {
                        let line = deckcraft_geom::preset::is_line_like(preset);
                        let rect = match preview.filter(|_| dragged) {
                            Some(x) => x,
                            None if line => Xfrm::new(start.x, start.y, 72.0, 0.0),
                            None => Xfrm::new(start.x, start.y, 72.0, 72.0),
                        };
                        let r = self.execute("shape.insert", &json!({"preset": preset, "rect": [rect.x, rect.y, rect.w, rect.h], "flip": format!("{}{}", if rect.flip_h { "h" } else { "" }, if rect.flip_v { "v" } else { "" })}))?;
                        if let (Some(id), Some(x)) = (r.get("id").and_then(Value::as_u64), preview.filter(|_| dragged && line)) {
                            let id = ShapeId(id as u32);
                            let _ = self.set_xfrm_cmd(id, x);
                            // Glue the ends that landed on connection sites.
                            let (p0, p1) = crate::connect::endpoints(&x);
                            let (g0, g1) = (self.glue_at(p0, 4.0), self.glue_at(p1, 4.0));
                            if g0.is_some() || g1.is_some() {
                                let _ = self.edit(|doc, sel| {
                                    if let Some(list) = crate::shapes_mut(doc, sel) {
                                        crate::connect::set_glue(list, id, 0, g0.map(|g| (g.0, g.1)));
                                        crate::connect::set_glue(list, id, 1, g1.map(|g| (g.0, g.1)));
                                    }
                                    Ok(())
                                });
                            }
                        }
                        r
                    }
                    ToolKind::TextBox => {
                        let w = preview.filter(|_| dragged).map(|x| x.w).unwrap_or(0.0);
                        let at = preview.filter(|_| dragged).map(|x| Point::new(x.x, x.y)).unwrap_or(start);
                        self.execute("insert.textBox", &json!({"rect": [at.x, at.y, w, 30.0]}))?
                    }
                    _ => Value::Null,
                };
                if !self.tool.sticky {
                    self.tool.kind = ToolKind::Select;
                }
                out = res;
            }
            Some(Drag::Freeform { mode, points, .. }) => {
                if mode == "scribble" {
                    // A scribble ending back at its start closes into a filled shape.
                    let closed = points.len() > 3 && points.first().zip(points.last()).is_some_and(|(a, b)| (*a - *b).hypot() <= 6.0);
                    self.tool.drag = Some(Drag::Freeform { mode, points, pressed: false });
                    return self.finish_freeform(closed);
                }
                // Polygon / curve: keep going until double-click, Enter or a click on the start.
                self.tool.drag = Some(Drag::Freeform { mode, points, pressed: false });
                return Ok(Value::Null);
            }
            Some(Drag::Ink { points }) => {
                self.tool.ink_preview.clear();
                if points.len() > 1 {
                    self.begin("Ink");
                    out = self.add_ink(points)?;
                }
            }
            Some(Drag::Resize { id, .. }) => {
                self.commit();
                let _ = cmd::text::fit_text_box(self, id);
                self.refit_after_resize(id)?;
            }
            Some(_) => self.commit(),
            None => {}
        }
        // Any open interaction (move etc.) ends here.
        self.commit();
        let _ = mods;
        Ok(out)
    }

    fn double_click(&mut self, p: Point, tol: f64) -> Result<Value> {
        if let Some(Drag::Freeform { mode, mut points, .. }) = self.tool.drag.clone() {
            // The double-click's own click added a duplicate vertex.
            while points.len() > 2 && points.last().zip(points.get(points.len() - 2)).is_some_and(|(a, b)| (*a - *b).hypot() <= tol) {
                points.pop();
            }
            self.tool.drag = Some(Drag::Freeform { mode, points, pressed: false });
            return self.finish_freeform(false);
        }
        if !matches!(self.tool.kind, ToolKind::Select) {
            // Double-click while drawing: lock drawing mode off.
            self.tool.sticky = false;
            return Ok(Value::Null);
        }
        let st = self.doc()?;
        if st.selection.text.is_some() {
            return self.execute("text.selectWord", &json!({}));
        }
        match self.hit_test(p, tol) {
            Some(Hit::Shape { id, .. }) => {
                let st = self.doc()?;
                let Some(sh) = st.shape(id).cloned() else { return Ok(Value::Null) };
                match &sh.kind {
                    ShapeKind::Table(_) => {
                        let x = xfrm_of(&st.doc, &st.selection, &sh);
                        let local = x.affine().inverse() * p;
                        if let ShapeKind::Table(t) = &sh.kind {
                            let cell = deckcraft_render_cell(t, local);
                            return self.execute("text.edit", &json!({"id": id, "cell": [cell.0, cell.1]}));
                        }
                        Ok(Value::Null)
                    }
                    ShapeKind::Shape | ShapeKind::Connector { .. } if !sh.is_line() => {
                        let pos = text_pos(st, &sh, p);
                        self.execute("text.edit", &json!({"id": id, "at": [pos.0, pos.1]}))?;
                        self.execute("text.selectWord", &json!({}))
                    }
                    ShapeKind::Group { .. } => Ok(Value::Null),
                    _ => {
                        self.ui_requests.push(crate::UiRequest::Pane { id: "format".into() });
                        Ok(Value::Null)
                    }
                }
            }
            _ => Ok(Value::Null),
        }
    }

    fn is_picture(&self, id: ShapeId) -> bool {
        self.active().and_then(|d| d.shape(id)).is_some_and(|s| matches!(s.kind, ShapeKind::Picture { .. } | ShapeKind::Media(_)))
    }

    /// End the freeform being drawn: create the shape (if it has at least two points).
    pub fn finish_freeform(&mut self, closed: bool) -> Result<Value> {
        let Some(Drag::Freeform { mode, points, .. }) = self.tool.drag.take() else { return Ok(Value::Null) };
        self.tool.path_preview.clear();
        self.tool.path_closing = false;
        if !self.tool.sticky {
            self.tool.kind = ToolKind::Select;
        }
        if points.len() < 2 {
            return Ok(Value::Null);
        }
        let pts: Vec<[f64; 2]> = points.iter().map(|q| [q.x, q.y]).collect();
        self.execute("shape.freeform", &json!({"points": pts, "closed": closed, "smooth": mode == "curve"}))
    }

    /// The connection site within glue range of `p` (excluding shape `exclude`).
    fn glue_at(&self, p: Point, tol: f64) -> Option<(ShapeId, u32, Point)> {
        let st = self.active()?;
        crate::connect::nearest_site(&st.doc, &st.selection, st.shapes(), p, (tol * 2.0).max(6.0), None)
    }

    /// Show the sites of the shape near `p` and return the site an end at `p` would glue to.
    fn update_sites(&mut self, p: Point, tol: f64, exclude: Option<ShapeId>) -> Option<(ShapeId, u32, Point)> {
        let (sites, glue) = match self.active() {
            Some(st) => {
                let shapes = st.shapes();
                let near = crate::connect::site_shape_at(&st.doc, &st.selection, shapes, p, tol * 3.0, exclude);
                let sites = near
                    .and_then(|id| shapes.iter().find(|s| s.id == id))
                    .map(|s| crate::connect::sites(&st.doc, &st.selection, s))
                    .unwrap_or_default();
                (sites, crate::connect::nearest_site(&st.doc, &st.selection, shapes, p, (tol * 2.0).max(6.0), exclude))
            }
            None => (vec![], None),
        };
        self.tool.sites = sites;
        self.tool.glue = glue.map(|g| g.2);
        glue
    }

    fn set_xfrm(&mut self, id: ShapeId, x: Xfrm) -> Result<()> {
        self.edit(|doc, sel| {
            let list = crate::shapes_mut(doc, sel).ok_or_else(|| cmd::bad("tool", "no slide"))?;
            if let Some(sh) = deckcraft_model::find_shape_mut(list, id) {
                sh.xfrm = Some(x);
                if let ShapeKind::Table(t) = &mut sh.kind {
                    // Tables resize their columns and rows proportionally.
                    let (tw, th) = (t.width().max(1e-6), t.height().max(1e-6));
                    let (kx, ky) = (x.w / tw, x.h / th);
                    t.cols.iter_mut().for_each(|c| *c *= kx);
                    t.rows.iter_mut().for_each(|r| r.height *= ky);
                }
            }
            Ok(())
        })
    }

    fn set_xfrm_cmd(&mut self, id: ShapeId, x: Xfrm) -> Result<Value> {
        self.execute("shape.setBounds", &json!({"id": id, "x": x.x, "y": x.y, "w": x.w, "h": x.h}))?;
        if x.flip_h || x.flip_v {
            self.edit(|doc, sel| {
                if let Some(sh) = crate::shapes_mut(doc, sel).and_then(|l| deckcraft_model::find_shape_mut(l, id))
                    && let Some(xx) = sh.xfrm.as_mut()
                {
                    xx.flip_h = x.flip_h;
                    xx.flip_v = x.flip_v;
                }
                Ok(())
            })?;
        }
        Ok(Value::Null)
    }

    fn refit_after_resize(&mut self, id: ShapeId) -> Result<()> {
        // A text box resized by hand keeps wrapping at its new width.
        let st = self.doc()?;
        if st.shape(id).is_some_and(|s| s.text_box) {
            let _ = cmd::text::fit_text_box(self, id);
        }
        Ok(())
    }

    fn execute_dup_in_place(&mut self, ids: &[ShapeId]) -> Result<()> {
        self.edit(|doc, sel| {
            let list = cmd::current_shapes(doc, sel).cloned().unwrap_or_default();
            let mut copies: Vec<Shape> = list.iter().filter(|x| ids.contains(&x.id)).cloned().collect();
            let boxes: Vec<Xfrm> = copies.iter().map(|c| xfrm_of(doc, sel, c)).collect();
            cmd::edit::reid(doc, &mut copies);
            for (c, b) in copies.iter_mut().zip(boxes) {
                c.xfrm = Some(b);
                c.ph = None;
            }
            let new_ids: Vec<ShapeId> = copies.iter().map(|c| c.id).collect();
            let shapes = crate::shapes_mut(doc, sel).ok_or_else(|| cmd::bad("duplicate", "no slide"))?;
            shapes.extend(copies);
            sel.shapes = new_ids;
            Ok(())
        })
    }

    fn add_ink(&mut self, points: Vec<(f64, f64, f32)>) -> Result<Value> {
        let ToolKind::Ink { mode, color, width } = self.tool.kind.clone() else { return Ok(Value::Null) };
        let stroke = deckcraft_model::InkStroke { points, color, width, highlighter: mode == "highlighter" };
        // Strokes join the slide's ink layer (one ink shape per slide).
        let existing = self.doc()?.shapes().iter().rev().find(|s| matches!(s.kind, ShapeKind::Ink { .. })).map(|s| s.id);
        self.edit(|doc, sel| {
            let id = match existing {
                Some(id) => id,
                None => {
                    let id = ShapeId(doc.alloc_id());
                    let size = doc.slide_size;
                    let list = crate::shapes_mut(doc, sel).ok_or_else(|| cmd::bad("ink", "no slide"))?;
                    list.push(Shape {
                        id,
                        name: "Ink".into(),
                        xfrm: Some(Xfrm::new(0.0, 0.0, size.width, size.height)),
                        kind: ShapeKind::Ink { strokes: vec![] },
                        ..Default::default()
                    });
                    id
                }
            };
            let list = crate::shapes_mut(doc, sel).ok_or_else(|| cmd::bad("ink", "no slide"))?;
            if let Some(sh) = deckcraft_model::find_shape_mut(list, id)
                && let ShapeKind::Ink { strokes } = &mut sh.kind
            {
                strokes.push(stroke);
            }
            Ok(json!({"ink": id}))
        })
    }

    fn erase_at(&mut self, p: Point, tol: f64) -> Result<Value> {
        let st = self.doc()?;
        let Some(ink) = st.shapes().iter().rev().find(|s| matches!(s.kind, ShapeKind::Ink { .. })).map(|s| s.id) else { return Ok(Value::Null) };
        let before = st.doc.clone();
        let sel_before = st.selection.clone();
        self.edit(|doc, sel| {
            let list = crate::shapes_mut(doc, sel).ok_or_else(|| cmd::bad("ink", "no slide"))?;
            if let Some(sh) = deckcraft_model::find_shape_mut(list, ink)
                && let ShapeKind::Ink { strokes } = &mut sh.kind
            {
                strokes.retain(|s| !s.points.iter().any(|(x, y, _)| (Point::new(*x, *y) - p).hypot() <= tol * 2.0 + s.width));
            }
            Ok(())
        })?;
        if let Some(st) = self.active_mut() {
            crate::push_undo(st, HistoryEntry { label: "Erase".into(), doc: before, selection: sel_before });
        }
        Ok(Value::Null)
    }
}

fn is_text_first(st: &crate::DocState, id: ShapeId) -> bool {
    st.shape(id).is_some_and(|s| s.ph.is_some() || s.text_box)
}

fn contains_rect(outer: Rect, inner: Rect) -> bool {
    inner.x0 >= outer.x0 && inner.y0 >= outer.y0 && inner.x1 <= outer.x1 && inner.y1 <= outer.y1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_corner_and_flip() {
        let o = Xfrm::new(100.0, 100.0, 100.0, 50.0);
        let n = resize_box(&o, 4, Vec2::new(20.0, 10.0), false, false);
        assert!((n.x - 100.0).abs() < 1e-9 && (n.w - 120.0).abs() < 1e-9 && (n.h - 60.0).abs() < 1e-9);
        let n = resize_box(&o, 0, Vec2::new(10.0, 10.0), false, false);
        assert!((n.x - 110.0).abs() < 1e-9 && (n.w - 90.0).abs() < 1e-9);
        let n = resize_box(&o, 3, Vec2::new(-150.0, 0.0), false, false);
        assert!(n.flip_h && (n.w - 50.0).abs() < 1e-9 && (n.x - 50.0).abs() < 1e-9);
        let n = resize_box(&o, 4, Vec2::new(100.0, 0.0), true, false);
        assert!((n.w / n.h - 2.0).abs() < 1e-9);
        let n = resize_box(&o, 3, Vec2::new(10.0, 0.0), false, true);
        assert!((n.w - 120.0).abs() < 1e-9 && (n.x - 90.0).abs() < 1e-9);
    }

    #[test]
    fn resize_rotated_keeps_opposite_corner() {
        let o = Xfrm { x: 100.0, y: 100.0, w: 100.0, h: 50.0, rot: 90.0, ..Default::default() };
        let anchor = o.affine() * Point::new(0.0, 0.0);
        let n = resize_box(&o, 4, Vec2::new(-10.0, 30.0), false, false);
        let anchor2 = n.affine() * Point::new(0.0, 0.0);
        assert!((anchor - anchor2).hypot() < 1e-6, "{anchor:?} {anchor2:?}");
    }
}
