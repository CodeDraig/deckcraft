//! The DeckCraft renderer: draws slides, layouts and masters with `vello_cpu`.
//!
//! One code path serves the editor canvas, thumbnails, image/PDF-raster export and the slide show:
//! [`render_slide`] draws the background, the master and layout graphics, then the slide's shapes
//! (fills with theme shading, gradients, pictures, outlines with dashes and arrowheads, shadows,
//! glow and soft edges, text, tables, charts, groups), optionally with per-shape animation state.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod text;
use text::prompt_body;
pub use text::{draw_layout, draw_layout_with, draw_text};

mod stroke;
use stroke::{arrowheads, stroke};

mod geometry;
pub use geometry::{crop_dest, parse_path, shape_geometry};

mod image;
pub use image::Image;

mod chart;
mod images;
mod paint;
mod placed;
mod table;

use deckcraft_color::Rgba;
use deckcraft_geom::preset::{FillMode, Geometry};
use deckcraft_model::resolve::{self, Ctx};
use deckcraft_model::style::{Effects, Fill};
use deckcraft_model::text::TextBody;
use deckcraft_model::{PhType, Presentation, Shape, ShapeId, ShapeKind, Slide};
use deckcraft_text::{Fields, Opts};
use kurbo::{Affine, BezPath, Point, Rect, Shape as _, Vec2};
use vello_common::filter_effects::{EdgeMode, Filter, FilterPrimitive};
use vello_cpu::{RenderContext, Resources, peniko};

pub use images::decode as decode_image;
pub use placed::{Placed, PlacedLink, PlacedText, place_slide};

/// Largest pixmap side we render (vello_cpu uses u16 sizes).
pub const MAX_SIDE: u32 = 16_000;

/// Animation state of one shape during a slide show or preview.
#[derive(Clone, Debug)]
pub struct ShapeState {
    pub visible: bool,
    pub opacity: f64,
    /// Applied in slide space around the shape's centre: translation (pt), scale, rotation (deg).
    pub offset: Vec2,
    pub scale: (f64, f64),
    pub rotate: f64,
    /// Reveal clip in shape-local unit space (0..1): l, t, r, b.
    pub clip: Option<[f64; 4]>,
    /// Colour override for emphasis effects (fill).
    pub tint: Option<Rgba>,
    /// Per-paragraph states of by-paragraph text builds (paragraph index, state).
    pub paras: Vec<(usize, ParaState)>,
}

impl Default for ShapeState {
    fn default() -> Self {
        ShapeState { visible: true, opacity: 1.0, offset: Vec2::ZERO, scale: (1.0, 1.0), rotate: 0.0, clip: None, tint: None, paras: vec![] }
    }
}

/// Animation state of one paragraph, relative to its shape.
#[derive(Clone, Copy, Debug)]
pub struct ParaState {
    pub visible: bool,
    pub opacity: f64,
    /// Translation in shape-local points.
    pub offset: Vec2,
}

impl Default for ParaState {
    fn default() -> Self {
        ParaState { visible: true, opacity: 1.0, offset: Vec2::ZERO }
    }
}

pub struct RenderOpts<'a> {
    /// Pixels per point.
    pub scale: f64,
    /// Editor view: placeholder prompts and dashed placeholder outlines.
    pub edit: bool,
    /// Draw shapes marked hidden (Selection pane) — the editor hides them like the show does.
    pub show_hidden: bool,
    /// Per-shape animation state.
    pub state: Option<&'a dyn Fn(ShapeId) -> ShapeState>,
    /// Don't draw these shapes (being edited in place, drawn by the UI).
    pub skip: &'a [ShapeId],
    /// Hide the text of this shape (the UI draws it while editing).
    pub skip_text: Option<ShapeId>,
    /// Worker threads (0 = this thread; required when effects use filters).
    pub threads: u16,
    /// Translate the slide within the output (pixels) and output size override.
    pub offset: (f64, f64),
    pub size: Option<(u32, u32)>,
    /// Draw a background colour outside the slide (output larger than slide).
    pub clear: Option<Rgba>,
    pub date_text: Option<String>,
}

impl Default for RenderOpts<'_> {
    fn default() -> Self {
        RenderOpts {
            scale: 1.0,
            edit: false,
            show_hidden: false,
            state: None,
            skip: &[],
            skip_text: None,
            threads: 0,
            offset: (0.0, 0.0),
            size: None,
            clear: None,
            date_text: None,
        }
    }
}

struct SlideFields {
    num: u32,
    date: String,
    footer: String,
}

impl Fields for SlideFields {
    fn field(&self, kind: &str) -> Option<String> {
        if kind == "slidenum" {
            return Some(self.num.to_string());
        }
        if kind.starts_with("datetime") {
            return Some(self.date.clone());
        }
        if kind == "footer" {
            return Some(self.footer.clone());
        }
        None
    }
}

pub struct Renderer {
    resources: Resources,
}

impl Default for Renderer {
    fn default() -> Self {
        Renderer { resources: Resources::new() }
    }
}

thread_local! {
    static RENDERER: std::cell::RefCell<Renderer> = std::cell::RefCell::new(Renderer::default());
}

/// Does anything on this slide (or its layout/master graphics) use blur-based effects?
pub fn needs_filters(pres: &Presentation, slide: &Slide) -> bool {
    let Some(rctx) = Ctx::for_slide(pres, slide) else { return false };
    let mut any = false;
    let mut check = |shapes: &[Shape]| {
        deckcraft_model::walk(shapes, &mut |s, _| {
            if any {
                return;
            }
            let (e, _) = resolve::effects(&rctx, s);
            if e.is_some_and(|e| !e.is_empty()) {
                any = true;
            }
        });
    };
    check(&slide.shapes);
    if let Some(l) = rctx.layout {
        check(&l.shapes);
    }
    check(&rctx.master.shapes);
    any
}

/// Render slide `index` at `opts.scale` pixels per point. A panic inside the renderer becomes a
/// blank image (logged), never a crash.
pub fn render_slide(pres: &Presentation, index: usize, opts: &RenderOpts) -> Image {
    let Some(slide) = pres.slides.get(index) else { return Image::default() };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| RENDERER.with(|r| r.borrow_mut().slide(pres, slide, index, opts)))) {
        Ok(img) => img,
        Err(_) => {
            log::error!("rendering slide {index} panicked; showing a blank slide");
            RENDERER.with(|r| *r.borrow_mut() = Renderer::default());
            let (w, h) = output_size(pres, opts);
            Image { width: w as u32, height: h as u32, pixels: vec![255; w as usize * h as usize * 4] }
        }
    }
}

/// Render a layout (Slide Master view thumbnail and canvas).
pub fn render_layout(pres: &Presentation, master: usize, layout: Option<usize>, opts: &RenderOpts) -> Image {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| RENDERER.with(|r| r.borrow_mut().layout(pres, master, layout, opts)))) {
        Ok(img) => img,
        Err(_) => {
            log::error!("rendering a layout panicked; showing a blank image");
            RENDERER.with(|r| *r.borrow_mut() = Renderer::default());
            Image::default()
        }
    }
}

fn output_size(pres: &Presentation, opts: &RenderOpts) -> (u16, u16) {
    let (w, h) = opts
        .size
        .unwrap_or(((pres.slide_size.width * opts.scale).round().max(1.0) as u32, (pres.slide_size.height * opts.scale).round().max(1.0) as u32));
    (w.clamp(1, MAX_SIDE) as u16, h.clamp(1, MAX_SIDE) as u16)
}

pub(crate) fn color(c: Rgba, alpha: f64) -> peniko::Color {
    let a = (c.a as f64 * alpha.clamp(0.0, 1.0)).round().clamp(0.0, 255.0) as u8;
    peniko::Color::from_rgba8(c.r, c.g, c.b, a)
}

fn shade(c: Rgba, m: FillMode) -> Rgba {
    match m {
        FillMode::Lighten => c.lerp(Rgba::WHITE.with_alpha(c.a), 0.4),
        FillMode::LightenLess => c.lerp(Rgba::WHITE.with_alpha(c.a), 0.2),
        FillMode::Darken => c.lerp(Rgba::BLACK.with_alpha(c.a), 0.4),
        FillMode::DarkenLess => c.lerp(Rgba::BLACK.with_alpha(c.a), 0.2),
        _ => c,
    }
}

struct Frame<'a> {
    pres: &'a Presentation,
    opts: &'a RenderOpts<'a>,
    fields: SlideFields,
}

impl Renderer {
    fn slide(&mut self, pres: &Presentation, slide: &Slide, index: usize, opts: &RenderOpts) -> Image {
        let (w, h) = output_size(pres, opts);
        let mut ctx = RenderContext::new_with(
            w,
            h,
            vello_cpu::RenderSettings {
                num_threads: if cfg!(target_arch = "wasm32") || needs_filters(pres, slide) { 0 } else { opts.threads },
                ..Default::default()
            },
        );
        if let Some(c) = opts.clear {
            ctx.set_paint(color(c, 1.0));
            ctx.fill_rect(&Rect::new(0.0, 0.0, w as f64, h as f64));
        }
        let view = Affine::translate((opts.offset.0, opts.offset.1)) * Affine::scale(opts.scale);
        let fields = SlideFields {
            num: pres.slide_number(index),
            date: opts.date_text.clone().unwrap_or_else(|| pres.header_footer.date_text.clone()),
            footer: pres.header_footer.footer_text.clone(),
        };
        let f = Frame { pres, opts, fields };
        if let Some(rctx) = Ctx::for_slide(pres, slide) {
            self.background(&mut ctx, &f, &rctx, Some(slide), view);
            let (show_layout, show_master) = resolve::show_master_shapes(slide, rctx.layout);
            if show_master {
                let mctx = Ctx::for_master(pres, rctx.master);
                for s in &rctx.master.shapes {
                    if s.ph.is_none() {
                        self.shape(&mut ctx, &f, &mctx, s, view, 0);
                    }
                }
            }
            if show_layout && let Some(l) = rctx.layout {
                let lctx = Ctx::for_layout(pres, rctx.master, l);
                for s in &l.shapes {
                    if s.ph.is_none() {
                        self.shape(&mut ctx, &f, &lctx, s, view, 0);
                    }
                }
            }
            for s in &slide.shapes {
                self.shape(&mut ctx, &f, &rctx, s, view, 0);
            }
        }
        self.finish(ctx, w, h)
    }

    fn layout(&mut self, pres: &Presentation, mi: usize, li: Option<usize>, opts: &RenderOpts) -> Image {
        let (w, h) = output_size(pres, opts);
        let mut ctx = RenderContext::new_with(w, h, vello_cpu::RenderSettings { num_threads: 0, ..Default::default() });
        let view = Affine::translate((opts.offset.0, opts.offset.1)) * Affine::scale(opts.scale);
        let f = Frame { pres, opts, fields: SlideFields { num: 1, date: String::new(), footer: String::new() } };
        if let Some(m) = pres.masters.get(mi) {
            match li.and_then(|i| m.layouts.get(i)) {
                Some(l) => {
                    let c = Ctx::for_layout(pres, m, l);
                    self.background(&mut ctx, &f, &c, None, view);
                    if l.show_master_shapes {
                        let mc = Ctx::for_master(pres, m);
                        for s in m.shapes.iter().filter(|s| s.ph.is_none()) {
                            self.shape(&mut ctx, &f, &mc, s, view, 0);
                        }
                    }
                    for s in &l.shapes {
                        self.shape(&mut ctx, &f, &c, s, view, 0);
                    }
                }
                None => {
                    let c = Ctx::for_master(pres, m);
                    self.background(&mut ctx, &f, &c, None, view);
                    for s in &m.shapes {
                        self.shape(&mut ctx, &f, &c, s, view, 0);
                    }
                }
            }
        }
        self.finish(ctx, w, h)
    }

    fn finish(&mut self, mut ctx: RenderContext, w: u16, h: u16) -> Image {
        ctx.flush();
        let mut pixels = vec![0u8; w as usize * h as usize * 4];
        if let Some(pm) = vello_cpu::PixmapMut::new(w, h, &mut pixels) {
            ctx.render(pm, &mut self.resources);
        }
        Image { width: w as u32, height: h as u32, pixels }
    }

    fn background(&mut self, ctx: &mut RenderContext, f: &Frame, rctx: &Ctx, slide: Option<&Slide>, view: Affine) {
        let (fill, ph) = resolve::background(rctx, slide);
        let r = Rect::new(0.0, 0.0, f.pres.slide_size.width, f.pres.slide_size.height);
        ctx.set_transform(view);
        let path = r.to_path(0.1);
        paint::fill_path(ctx, rctx, &fill, ph, &path, r, view, FillMode::Norm, 1.0, f.pres);
    }

    fn shape(&mut self, ctx: &mut RenderContext, f: &Frame, rctx: &Ctx, s: &Shape, parent: Affine, depth: usize) {
        if depth > 32 || (s.hidden && !f.opts.show_hidden) || f.opts.skip.contains(&s.id) {
            return;
        }
        // Footer placeholders on layouts/masters show only on slides that include them.
        let st = f.opts.state.map(|g| g(s.id)).unwrap_or_default();
        if !st.visible || st.opacity <= 0.001 {
            return;
        }
        let x = resolve::xfrm(rctx, s);
        if !x.is_finite() {
            return;
        }
        let mut m = parent * x.affine();
        if st.offset != Vec2::ZERO || st.scale != (1.0, 1.0) || st.rotate != 0.0 {
            let c = Point::new(x.w / 2.0, x.h / 2.0);
            let around = Affine::translate(c.to_vec2())
                * Affine::rotate(st.rotate.to_radians())
                * Affine::scale_non_uniform(st.scale.0, st.scale.1)
                * Affine::translate(-c.to_vec2());
            m = parent * Affine::translate(st.offset) * x.affine() * around;
        }
        let layered = st.opacity < 0.999 || st.clip.is_some();
        if layered {
            ctx.set_transform(m);
            let clip = st.clip.map(|[l, t, r, b]| Rect::new(l * x.w, t * x.h, r * x.w, b * x.h).to_path(0.1));
            ctx.push_layer(clip.as_ref(), None, Some(st.opacity.clamp(0.0, 1.0) as f32), None, None);
        }
        match &s.kind {
            ShapeKind::Group { children, child } => {
                let ch = Rect::new(child.x, child.y, child.x + child.w, child.y + child.h);
                let gm = parent * deckcraft_geom::group_child_affine(&x, ch);
                for c in children {
                    self.shape(ctx, f, rctx, c, gm, depth + 1);
                }
            }
            ShapeKind::Table(t) => table::draw(ctx, f.pres, rctx, t, m, x.w, x.h, &f.fields),
            ShapeKind::Chart(c) => chart::draw(ctx, rctx, c, m, x.w, x.h),
            ShapeKind::Ink { strokes } => {
                for sk in strokes {
                    let mut bp = BezPath::new();
                    for (i, (px, py, _)) in sk.points.iter().enumerate() {
                        if i == 0 {
                            bp.move_to((*px, *py));
                        } else {
                            bp.line_to((*px, *py));
                        }
                    }
                    ctx.set_transform(parent);
                    ctx.set_paint(color(sk.color, if sk.highlighter { 0.4 } else { 1.0 }));
                    ctx.set_stroke(kurbo::Stroke::new(sk.width.max(0.1)).with_caps(kurbo::Cap::Round).with_join(kurbo::Join::Round));
                    ctx.stroke_path(&bp);
                }
            }
            _ => self.geometry_shape(ctx, f, rctx, s, m, x.w, x.h, &st),
        }
        if layered {
            ctx.pop_layer();
        }
    }

    fn geometry_shape(&mut self, ctx: &mut RenderContext, f: &Frame, rctx: &Ctx, s: &Shape, m: Affine, w: f64, h: f64, st: &ShapeState) {
        let geo = shape_geometry(s, w, h);
        let (fill, fill_ph) = resolve::fill(rctx, s);
        let (line, line_ph) = resolve::line(rctx, s);
        let (effects, fx_ph) = resolve::effects(rctx, s);
        let is_pic = matches!(s.kind, ShapeKind::Picture { .. });
        let outline = geo.outline();
        // Placeholders in the editor: dashed outline and prompt, nothing else when empty.
        let empty_ph = s.ph.is_some() && s.text.as_ref().is_none_or(TextBody::is_empty) && !is_pic && !matches!(s.kind, ShapeKind::Media(_));
        let fill = if let Some(t) = st.tint { Some(Fill::solid(deckcraft_model::ColorRef::rgb(t))) } else { fill };
        // Effects below the shape.
        if let Some(e) = &effects {
            self.effects_below(ctx, rctx, e, fx_ph, &geo, &fill, m, w, h);
        }
        let soft = effects.as_ref().and_then(|e| e.soft_edge).filter(|r| *r > 0.0);
        if let Some(r) = soft {
            ctx.set_transform(Affine::IDENTITY);
            ctx.push_layer(None, None, None, None, None);
            let _ = r;
        }
        // Fill.
        match &s.kind {
            ShapeKind::Picture { fill: pf } => {
                ctx.set_transform(m);
                let bounds = Rect::new(0.0, 0.0, w, h);
                ctx.push_clip_layer(&outline);
                paint::picture(ctx, f.pres, pf, bounds, m);
                ctx.pop_layer();
            }
            ShapeKind::Media(mc) => {
                ctx.set_transform(m);
                let bounds = Rect::new(0.0, 0.0, w, h);
                if let Some(p) = mc.poster {
                    let pf = deckcraft_model::style::PictureFill { media: p, ..Default::default() };
                    paint::picture(ctx, f.pres, &pf, bounds, m);
                } else if mc.video {
                    ctx.set_paint(peniko::Color::from_rgba8(20, 20, 24, 255));
                    ctx.fill_rect(&bounds);
                }
                if !mc.video || mc.poster.is_none() {
                    paint::media_glyph(ctx, mc.video, bounds);
                }
            }
            _ => {
                if let Some(fl) = &fill
                    && !(empty_ph && f.opts.edit && s.fill.is_none())
                {
                    for sp in &geo.paths {
                        if sp.fill == FillMode::None {
                            continue;
                        }
                        ctx.set_transform(m);
                        ctx.set_fill_rule(if sp.even_odd { peniko::Fill::EvenOdd } else { peniko::Fill::NonZero });
                        paint::fill_path(ctx, rctx, fl, fill_ph, &sp.path, Rect::new(0.0, 0.0, w, h), m, sp.fill, 1.0, f.pres);
                        ctx.set_fill_rule(peniko::Fill::NonZero);
                    }
                }
            }
        }
        // Outline.
        if line.fill.as_ref().is_some_and(|f| !f.is_none()) {
            let lw = line.width.unwrap_or(0.75).max(0.0);
            for sp in geo.paths.iter().filter(|p| p.stroke) {
                stroke(ctx, rctx, &line, line_ph, &sp.path, m, lw);
            }
            if s.is_line() || geo.is_open() {
                arrowheads(ctx, rctx, &line, line_ph, &geo, m, lw);
            }
        }
        if let Some(r) = soft {
            // Feather: fade the shape's alpha towards the outline.
            ctx.set_transform(m);
            ctx.push_layer(
                None,
                Some(peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::DestIn)),
                None,
                None,
                Some(blur(r * 0.5 * scale_of(m))),
            );
            ctx.set_paint(peniko::Color::from_rgba8(0, 0, 0, 255));
            let inset = Rect::new(0.0, 0.0, w, h).inset(-r * 0.6);
            ctx.fill_rect(&inset);
            ctx.pop_layer();
            ctx.pop_layer();
        }
        // Inner shadow.
        if let Some(e) = &effects
            && let Some(sh) = &e.inner_shadow
        {
            ctx.set_transform(m);
            ctx.push_clip_layer(&outline);
            let c = rctx.color(&sh.color, fx_ph);
            let d = Vec2::new(sh.dir.to_radians().cos(), sh.dir.to_radians().sin()) * sh.dist;
            ctx.push_layer(None, None, None, None, Some(blur(sh.blur * 0.5 * scale_of(m))));
            ctx.set_paint(color(c, 1.0));
            let mut ring = Rect::new(0.0, 0.0, w, h).inflate(w.max(h), w.max(h)).to_path(0.1);
            let mut hole = outline.clone();
            hole.apply_affine(Affine::translate(d));
            ring.extend(hole.iter());
            ctx.set_fill_rule(peniko::Fill::EvenOdd);
            ctx.fill_path(&ring);
            ctx.set_fill_rule(peniko::Fill::NonZero);
            ctx.pop_layer();
            ctx.pop_layer();
        }
        // Text (or the placeholder prompt in the editor).
        let skip_text = f.opts.skip_text == Some(s.id);
        if let Some(body) = &s.text
            && !skip_text
        {
            let tr = geo.text_rect;
            if !body.is_empty() {
                let l = deckcraft_text::layout(rctx, s, body, &Opts { rect: tr, fields: &f.fields, prompt_color: None, no_shrink: false });
                draw_layout_with(ctx, &l, tr, m, &st.paras);
            } else if f.opts.edit && s.ph.is_some() {
                let kind = s.ph_type().unwrap_or(PhType::Body);
                let prompt_body = prompt_body(rctx, s, kind);
                if !prompt_body.is_empty() {
                    let grey = Rgba::rgb(0x59, 0x59, 0x59);
                    draw_text(ctx, rctx, s, &prompt_body, tr, m, &f.fields, Some(grey));
                }
            }
        }
        if empty_ph && f.opts.edit {
            // Dashed placeholder outline.
            ctx.set_transform(m);
            let px = 1.0 / scale_of(m).max(1e-6);
            ctx.set_paint(peniko::Color::from_rgba8(0xA6, 0xA6, 0xA6, 255));
            ctx.set_stroke(kurbo::Stroke::new(px).with_dashes(0.0, [3.0 * px, 2.0 * px]));
            ctx.stroke_path(&Rect::new(0.0, 0.0, w, h).to_path(0.1));
            if matches!(s.ph_type(), Some(PhType::Picture | PhType::Chart | PhType::Table | PhType::Media | PhType::Diagram)) {
                paint::placeholder_icon(ctx, s.ph_type().unwrap_or(PhType::Picture), Rect::new(0.0, 0.0, w, h));
            }
        }
    }

    fn effects_below(
        &mut self,
        ctx: &mut RenderContext,
        rctx: &Ctx,
        e: &Effects,
        ph: Option<Rgba>,
        geo: &Geometry,
        fill: &Option<Fill>,
        m: Affine,
        w: f64,
        h: f64,
    ) {
        let silhouette = {
            let mut p = BezPath::new();
            for sp in &geo.paths {
                if sp.fill != FillMode::None || fill.is_none() {
                    p.extend(sp.path.iter());
                }
            }
            if p.elements().is_empty() { geo.outline() } else { p }
        };
        let k = scale_of(m);
        if let Some(sh) = &e.outer_shadow {
            let c = rctx.color(&sh.color, ph);
            let d = Vec2::new(sh.dir.to_radians().cos(), sh.dir.to_radians().sin()) * sh.dist;
            let c0 = Point::new(w / 2.0, h / 2.0);
            let local = Affine::translate(c0.to_vec2())
                * Affine::new([sh.sx, sh.ky.to_radians().tan(), sh.kx.to_radians().tan(), sh.sy, 0.0, 0.0])
                * Affine::translate(-c0.to_vec2());
            // Shadow offset is in slide space (doesn't rotate with the shape unless asked).
            let sm = if sh.rotate_with_shape { m * Affine::translate(d) * local } else { pre_translate(m, d * k) * local };
            ctx.set_transform(Affine::IDENTITY);
            ctx.push_layer(None, None, Some((c.a as f32) / 255.0), None, Some(blur(sh.blur * 0.5 * k)));
            ctx.set_transform(sm);
            ctx.set_paint(color(c.with_alpha(255), 1.0));
            ctx.fill_path(&silhouette);
            ctx.pop_layer();
        }
        if let Some(g) = &e.glow {
            let c = rctx.color(&g.color, ph);
            ctx.set_transform(Affine::IDENTITY);
            ctx.push_layer(None, None, Some((c.a as f32) / 255.0), None, Some(blur(g.radius * 0.35 * k)));
            ctx.set_transform(m);
            ctx.set_paint(color(c.with_alpha(255), 1.0));
            ctx.set_stroke(kurbo::Stroke::new(g.radius.max(0.0) * 1.2).with_join(kurbo::Join::Round));
            ctx.stroke_path(&silhouette);
            ctx.fill_path(&silhouette);
            ctx.pop_layer();
        }
        if let Some(rf) = &e.reflection {
            // Mirror below the shape, fading out.
            let mm = m * Affine::translate((0.0, 2.0 * h + rf.dist)) * Affine::scale_non_uniform(1.0, -1.0);
            ctx.set_transform(Affine::IDENTITY);
            ctx.push_layer(None, None, Some(rf.start_alpha.clamp(0.0, 1.0) as f32 * 0.6), None, Some(blur(rf.blur * 0.5 * k)));
            ctx.set_transform(mm);
            if let Some(fl) = fill {
                paint::fill_path(ctx, rctx, fl, ph, &silhouette, Rect::new(0.0, 0.0, w, h), mm, FillMode::Norm, 1.0, rctx.pres);
            }
            // Fade mask.
            ctx.push_layer(None, Some(peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::DestIn)), None, None, None);
            let grad = peniko::Gradient::new_linear(Point::new(0.0, h), Point::new(0.0, h * (1.0 - rf.end_pos.clamp(0.01, 1.0))))
                .with_stops([(0.0, peniko::Color::from_rgba8(0, 0, 0, 255)), (1.0, peniko::Color::from_rgba8(0, 0, 0, 0))].as_slice());
            ctx.set_paint(grad);
            ctx.fill_rect(&Rect::new(0.0, 0.0, w, h).inflate(4.0, 4.0));
            ctx.pop_layer();
            ctx.pop_layer();
        }
    }
}

fn pre_translate(m: Affine, d: Vec2) -> Affine {
    Affine::translate(d) * m
}

pub(crate) fn scale_of(m: Affine) -> f64 {
    m.determinant().abs().sqrt()
}

fn blur(sigma: f64) -> Filter {
    Filter::from_primitive(FilterPrimitive::GaussianBlur { std_deviation: sigma.clamp(0.0, 500.0) as f32, edge_mode: EdgeMode::None })
}

/// Render one shape alone onto a transparent canvas (drag previews, copy as picture).
pub fn render_shape(pres: &Presentation, slide: &Slide, id: ShapeId, scale: f64) -> Option<Image> {
    let rctx = Ctx::for_slide(pres, slide)?;
    let s = slide.shape(id)?;
    let x = resolve::xfrm(&rctx, s);
    let b = x.bounds().inflate(8.0, 8.0);
    let w = (b.width() * scale).ceil().clamp(1.0, MAX_SIDE as f64) as u16;
    let h = (b.height() * scale).ceil().clamp(1.0, MAX_SIDE as f64) as u16;
    let mut ctx = RenderContext::new_with(w, h, vello_cpu::RenderSettings { num_threads: 0, ..Default::default() });
    let opts = RenderOpts { scale, ..Default::default() };
    let f = Frame { pres, opts: &opts, fields: SlideFields { num: 1, date: String::new(), footer: String::new() } };
    let view = Affine::scale(scale) * Affine::translate(-b.origin().to_vec2());
    RENDERER.with(|r| {
        let mut r = r.borrow_mut();
        r.shape(&mut ctx, &f, &rctx, s, view, 0);
        Some(r.finish(ctx, w, h))
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mt_tests {
    use super::*;

    /// Regression: multithreaded rendering of a slide with shadows panicked inside vello_cpu.
    #[test]
    fn effects_render_with_threads_requested() {
        let mut s = deckcraft_model::Presentation::default();
        let sl = std::sync::Arc::make_mut(&mut s.slides[0]);
        let mut sh = deckcraft_model::Shape {
            id: ShapeId(900),
            xfrm: Some(deckcraft_geom::Xfrm::new(10.0, 10.0, 100.0, 100.0)),
            style: Some(deckcraft_model::ShapeStyle::accent(deckcraft_color::SchemeSlot::Accent1)),
            ..Default::default()
        };
        sh.effects = Some(deckcraft_model::Effects { soft_edge: Some(5.0), ..Default::default() });
        sl.shapes.push(sh);
        assert!(needs_filters(&s, &s.slides[0]));
        let img = RENDERER.with(|r| r.borrow_mut().slide(&s, &s.slides[0], 0, &RenderOpts { scale: 0.5, threads: 4, ..Default::default() }));
        assert_eq!(img.width, 480);
        // The shape itself is drawn (not a blank fallback).
        assert_ne!(img.pixel(30, 30), [255, 255, 255, 255]);
    }
}
