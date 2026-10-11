//! Designer task pane.

use crate::SlideApp;
use crate::theme::Tokens;
use egui::{Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

/// Design Ideas: offline suggestions — the current slide in each theme and layout variations.
pub(super) fn designer(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new("Pick a look for your slides. Ideas are generated on your device.").color(t.text_dim));
    ui.add_space(6.0);
    let Some(st) = app.session.active() else { return };
    let doc = st.doc.clone();
    let idx = st.selection.slide;
    let w = ui.available_width() - 8.0;
    for th in deckcraft_model::theme::builtin_themes() {
        let mut d = (*doc).clone();
        if let Some(m) = d.masters.first_mut() {
            let m = std::sync::Arc::make_mut(m);
            m.theme = th.clone();
        }
        let ppp = ui.ctx().pixels_per_point();
        let px = (w * ppp) as u32;
        let h = (px as f64 * d.slide_size.height / d.slide_size.width.max(1.0)) as u32;
        let img = deckcraft_render::render_slide(
            &d,
            idx,
            &deckcraft_render::RenderOpts { scale: px as f64 / d.slide_size.width.max(1.0), size: Some((px, h)), ..Default::default() },
        );
        let tex = ui.ctx().load_texture(format!("idea-{}", th.name), crate::textures::to_color_image(&img, false), egui::TextureOptions::LINEAR);
        let (r, resp) = ui.allocate_exact_size(vec2(w, w * h as f32 / px.max(1) as f32), Sense::click());
        ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        ui.painter().rect_stroke(
            r,
            CornerRadius::same(2),
            Stroke::new(if resp.hovered() { 2.5 } else { 1.0 }, if resp.hovered() { t.accent } else { t.border }),
            egui::StrokeKind::Outside,
        );
        if resp.on_hover_text(&th.name).clicked() {
            let _ = app.run("design.theme", json!({"name": th.name}));
        }
        ui.add_space(8.0);
    }
}
