//! The slide canvas: the rendered slide, zoom and fit, pointer input routed to the engine's tools,
//! and the overlays drawn on top (selection box and handles, smart guides, marquee, drawing
//! previews, the text caret and text selection).

mod pointer;
use pointer::{context_menu, pointer};

mod selection;
use selection::overlays;

mod media;
use media::{media_bar, media_frames};

use deckcraft_engine::Target;
use deckcraft_geom::Point;
use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

use crate::SlideApp;
use crate::theme::{self, Tokens};

/// Slide-to-screen mapping.
#[derive(Clone, Copy, Debug)]
pub struct Xf {
    pub origin: Pos2,
    pub scale: f32,
}

impl Xf {
    pub fn to_screen(&self, p: Point) -> Pos2 {
        pos2(self.origin.x + p.x as f32 * self.scale, self.origin.y + p.y as f32 * self.scale)
    }
    pub fn to_slide(&self, p: Pos2) -> Point {
        Point::new(((p.x - self.origin.x) / self.scale) as f64, ((p.y - self.origin.y) / self.scale) as f64)
    }
}

/// The slide's rect on screen for the current zoom.
pub fn layout(app: &SlideApp, avail: Rect) -> (Rect, f32) {
    let Some(d) = app.session.active() else { return (avail, 1.0) };
    let (sw, sh) = (d.doc.slide_size.width as f32, d.doc.slide_size.height as f32);
    let margin = 28.0;
    let fit = ((avail.width() - margin * 2.0) / sw).min((avail.height() - margin * 2.0) / sh).max(0.05);
    let scale = app.ui.zoom.unwrap_or(fit);
    let size = vec2(sw * scale, sh * scale);
    let mut r = Rect::from_center_size(avail.center(), size);
    if size.x > avail.width() {
        r = Rect::from_min_size(pos2(avail.min.x + margin, r.min.y), size);
    }
    if size.y > avail.height() {
        r = Rect::from_min_size(pos2(r.min.x, avail.min.y + margin), size);
    }
    (r, scale)
}

pub fn show(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let avail = ui.available_rect_before_wrap();
    app.canvas_rect = Some(avail);
    let Some(st) = app.session.active() else { return };
    if st.doc.slides.is_empty() && st.selection.target == Target::Slides {
        let resp = ui.allocate_rect(avail, Sense::click());
        ui.painter().text(avail.center(), Align2::CENTER_CENTER, "Click to add first slide", theme::font(22.0), t.text_dim);
        if resp.clicked() {
            let _ = app.run("slide.new", json!({"layout": "title"}));
        }
        return;
    }
    let (content_rect, scale) = layout(app, avail);
    // Scroll when zoomed beyond the window.
    let scroll_id = ui.id().with("canvas_scroll");
    let mut offset: egui::Vec2 = ui.data_mut(|d| d.get_temp(scroll_id).unwrap_or_default());
    let overflow = vec2((content_rect.width() + 56.0 - avail.width()).max(0.0), (content_rect.height() + 56.0 - avail.height()).max(0.0));
    let resp = ui.allocate_rect(avail, Sense::click_and_drag());
    let (wheel, zoom_delta, pinch_pos) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
    if resp.hovered() {
        if (zoom_delta - 1.0).abs() > 1e-3 {
            let cur = app.ui.zoom.unwrap_or(scale);
            app.ui.zoom = Some((cur * zoom_delta).clamp(0.1, 4.0));
            let _ = pinch_pos;
        } else if overflow != egui::Vec2::ZERO {
            offset -= wheel;
        } else if wheel.y.abs() > 18.0 && app.session.tool.marquee.is_none() && !app.session.tool.dragging() {
            // Wheel over a fitted slide pages through slides (like PowerPoint).
            let id = if wheel.y < 0.0 { "slide.next" } else { "slide.previous" };
            let gate = ui.id().with("wheel_gate");
            let last: f64 = ui.data_mut(|d| d.get_temp(gate).unwrap_or(0.0));
            let now = ui.input(|i| i.time);
            if now - last > 0.35 {
                ui.data_mut(|d| d.insert_temp(gate, now));
                let _ = app.run(id, json!({}));
            }
        }
    }
    offset.x = offset.x.clamp(0.0, overflow.x);
    offset.y = offset.y.clamp(0.0, overflow.y);
    ui.data_mut(|d| d.insert_temp(scroll_id, offset));
    let slide_rect = content_rect.translate(-offset);
    app.slide_rect = Some(slide_rect);
    app.canvas_scale = scale;
    let xf = Xf { origin: slide_rect.min, scale };
    let painter = ui.painter_at(avail);
    // Shadow + slide.
    painter.rect_filled(slide_rect.translate(vec2(0.0, 1.5)).expand(1.0), CornerRadius::same(2), t.shadow);
    let ppp = ui.ctx().pixels_per_point();
    let px = ((slide_rect.width() * ppp).round().max(1.0) as u32, (slide_rect.height() * ppp).round().max(1.0) as u32);
    let st = app.session.active();
    let tex = match st.map(|d| (d.selection.target, d)) {
        Some((Target::Slides, d)) => {
            let idx = d.selection.slide;
            let doc = d.doc.clone();
            app.textures.slide(ui.ctx(), &doc, idx, px, true, 0, app.ui.grayscale)
        }
        Some((Target::Master { master }, d)) => master_tex(ui.ctx(), &d.doc, master, None, px),
        Some((Target::Layout { master, layout }, d)) => master_tex(ui.ctx(), &d.doc, master, Some(layout), px),
        None => None,
    };
    if let Some(tex) = tex {
        painter.image(tex.id(), slide_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    app.perf.render_ms = app.textures.last_render_ms;
    media_frames(app, &painter, xf);
    if app.ui.gridlines {
        let step = 72.0 / 2.0 * scale;
        let mut x = slide_rect.min.x + step;
        while x < slide_rect.max.x {
            painter.line_segment([pos2(x, slide_rect.min.y), pos2(x, slide_rect.max.y)], Stroke::new(0.5, Color32::from_black_alpha(40)));
            x += step;
        }
        let mut y = slide_rect.min.y + step;
        while y < slide_rect.max.y {
            painter.line_segment([pos2(slide_rect.min.x, y), pos2(slide_rect.max.x, y)], Stroke::new(0.5, Color32::from_black_alpha(40)));
            y += step;
        }
    }
    if app.ui.guides {
        let c = slide_rect.center();
        painter.line_segment(
            [pos2(c.x, slide_rect.min.y), pos2(c.x, slide_rect.max.y)],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 120, 120, 160)),
        );
        painter.line_segment(
            [pos2(slide_rect.min.x, c.y), pos2(slide_rect.max.x, c.y)],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 120, 120, 160)),
        );
    }
    if app.ui.ruler {
        ruler(&painter, avail, slide_rect, scale, &t);
    }
    pointer(app, ui, &resp, xf);
    overlays(app, ui, &painter, xf, &t);
    media_bar(app, ui, xf);
    context_menu(app, &resp);
}

fn master_tex(
    ctx: &egui::Context,
    doc: &deckcraft_model::Presentation,
    master: usize,
    layout: Option<usize>,
    px: (u32, u32),
) -> Option<egui::TextureHandle> {
    let img = deckcraft_render::render_layout(
        doc,
        master,
        layout,
        &deckcraft_render::RenderOpts { scale: px.0 as f64 / doc.slide_size.width.max(1.0), edit: true, size: Some(px), ..Default::default() },
    );
    Some(ctx.load_texture("master-canvas", crate::textures::to_color_image(&img, false), egui::TextureOptions::LINEAR))
}

fn ruler(p: &egui::Painter, avail: Rect, slide: Rect, scale: f32, t: &Tokens) {
    let top = Rect::from_min_max(avail.min, pos2(avail.max.x, avail.min.y + 16.0));
    let left = Rect::from_min_max(avail.min, pos2(avail.min.x + 16.0, avail.max.y));
    p.rect_filled(top, CornerRadius::ZERO, t.card);
    p.rect_filled(left, CornerRadius::ZERO, t.card);
    let inch = 72.0 * scale;
    let mut k = 0;
    loop {
        let x = slide.min.x + k as f32 * inch / 4.0;
        if x > slide.max.x {
            break;
        }
        let h = if k % 4 == 0 {
            8.0
        } else if k % 2 == 0 {
            5.0
        } else {
            3.0
        };
        p.line_segment([pos2(x, top.max.y - h), pos2(x, top.max.y)], Stroke::new(1.0, t.text_faint));
        if k % 4 == 0 {
            p.text(pos2(x + 2.0, top.min.y + 1.0), Align2::LEFT_TOP, format!("{}", k / 4), theme::font(8.0), t.text_dim);
        }
        k += 1;
    }
    let mut k = 0;
    loop {
        let y = slide.min.y + k as f32 * inch / 4.0;
        if y > slide.max.y {
            break;
        }
        let w = if k % 4 == 0 {
            8.0
        } else if k % 2 == 0 {
            5.0
        } else {
            3.0
        };
        p.line_segment([pos2(left.max.x - w, y), pos2(left.max.x, y)], Stroke::new(1.0, t.text_faint));
        k += 1;
    }
}
