//! Home tab and slide-layout selection.

use super::clipboard::clipboard_group;
use super::text::{font_group, paragraph_group};
use super::{big, drawing_group, enabled, fmt_state, small};
use crate::SlideApp;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{big_button_ex, group, rows, small_button_ex};
use egui::{Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

pub(super) fn layout_menu(app: &mut SlideApp, ui: &mut Ui, cmd: &str) {
    let layouts: Vec<(String, deckcraft_model::LayoutId)> = app
        .session
        .active()
        .and_then(|d| d.doc.masters.first().map(|m| m.layouts.iter().map(|l| (l.name.clone(), l.id)).collect()))
        .unwrap_or_default();
    egui::Grid::new(("layouts", cmd)).spacing(vec2(6.0, 6.0)).show(ui, |ui| {
        for (k, (name, id)) in layouts.iter().enumerate() {
            let idx = app.session.active().and_then(|d| d.doc.masters.first().and_then(|m| m.layouts.iter().position(|l| l.id == *id)));
            let resp = ui.vertical(|ui| {
                let (rect, resp) = ui.allocate_exact_size(vec2(96.0, 54.0), Sense::click());
                if let (Some(d), Some(li)) = (app.session.active(), idx) {
                    let img = deckcraft_render::render_layout(
                        &d.doc,
                        0,
                        Some(li),
                        &deckcraft_render::RenderOpts {
                            scale: 192.0 / d.doc.slide_size.width.max(1.0),
                            edit: true,
                            size: Some((192, 108)),
                            ..Default::default()
                        },
                    );
                    let tex =
                        ui.ctx().load_texture(format!("layout-{li}"), crate::textures::to_color_image(&img, false), egui::TextureOptions::LINEAR);
                    ui.painter().image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                let t = Tokens::get(ui.ctx());
                ui.painter().rect_stroke(
                    rect,
                    CornerRadius::ZERO,
                    Stroke::new(if resp.hovered() { 2.0 } else { 1.0 }, if resp.hovered() { t.accent } else { t.border }),
                    egui::StrokeKind::Outside,
                );
                ui.add_sized(vec2(96.0, 14.0), egui::Label::new(egui::RichText::new(name).size(10.5)).truncate());
                resp
            });
            if resp.inner.clicked() {
                let _ = app.run(cmd, json!({"layout": id.0}));
                ui.close();
            }
            if k % 4 == 3 {
                ui.end_row();
            }
        }
    });
}

pub(super) fn slides_group(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        let (main, arrow) = big_button_ex(ui, Icon::NewSlide, "New\nSlide", enabled(app, "slide.new"), true, false);
        if main.on_hover_text("New Slide (⇧⌘N)").clicked() {
            let _ = app.run("slide.new", json!({}));
        }
        if let Some(a) = arrow {
            egui::Popup::menu(&a).width(420.0).show(|ui| {
                layout_menu(app, ui, "slide.new");
                ui.separator();
                if ui.button("Duplicate Selected Slides").clicked() {
                    let _ = app.run("slide.duplicate", json!({}));
                }
                if ui.button("Slides from Outline…").clicked() {
                    app.dialog = Some(crate::dialogs::Dialog::new("outline"));
                }
            });
        }
        rows(ui, |ui| {
            let (r, _) = small_button_ex(ui, Icon::Layout, "Layout", enabled(app, "slide.layout"), false, true);
            egui::Popup::menu(&r).width(420.0).show(|ui| layout_menu(app, ui, "slide.layout"));
            small(app, ui, Icon::ResetSlide, "Reset", "slide.reset", json!({}));
            let (r, _) = small_button_ex(ui, Icon::Section, "Section", enabled(app, "section.add"), false, true);
            egui::Popup::menu(&r).show(|ui| {
                if ui.button("Add Section").clicked() {
                    let _ = app.run("section.add", json!({}));
                }
                if ui.button("Remove All Sections").clicked() {
                    let _ = app.run("section.removeAll", json!({}));
                }
            });
        });
    });
}

pub(super) fn home(app: &mut SlideApp, ui: &mut Ui) {
    let st = fmt_state(app);
    clipboard_group(app, ui);
    slides_group(app, ui);
    font_group(app, ui, &st);
    paragraph_group(app, ui, &st);
    group(ui, |ui| {
        big(app, ui, Icon::Picture, "Picture", "app.insertPictureDialog", json!({}));
    });
    drawing_group(app, ui);
    group(ui, |ui| {
        big(app, ui, Icon::DesignIdeas, "Design\nIdeas", "view.pane", json!({"pane": "designer"}));
    });
}
