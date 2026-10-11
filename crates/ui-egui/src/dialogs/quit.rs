//! Unsaved-document close confirmation.

use super::Dialog;
use crate::SlideApp;
use crate::theme::Tokens;
use egui::{Ui, vec2};
use serde_json::json;

pub(super) fn quit_dialog(app: &mut SlideApp, ui: &mut Ui, _d: &mut Dialog) -> bool {
    let names: Vec<String> = app.session.documents().iter().filter(|d| d.is_dirty()).map(|d| d.title()).collect();
    if names.is_empty() {
        app.quit_confirmed = true;
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        return true;
    }
    let what = if names.len() == 1 { format!("“{}”", names[0]) } else { format!("{} presentations", names.len()) };
    ui.label(egui::RichText::new(format!("Do you want to save the changes you made to {what}?")).strong());
    ui.label("Your changes will be lost if you don't save them.");
    ui.add_space(8.0);
    let mut done = false;
    ui.horizontal(|ui| {
        let t = Tokens::get(ui.ctx());
        if ui.add(egui::Button::new("Don't Save").min_size(vec2(84.0, 24.0))).clicked() {
            let _ = app.session.execute("file.recovery.discard", &json!({}));
            app.quit_confirmed = true;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            done = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let save = ui.add(egui::Button::new(egui::RichText::new("Save").color(t.accent_text)).fill(t.accent).min_size(vec2(72.0, 24.0)));
            if save.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if app.save_all() {
                    app.quit_confirmed = true;
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                done = true;
            }
            if ui.add(egui::Button::new("Cancel").min_size(vec2(72.0, 24.0))).clicked() {
                done = true;
            }
        });
    });
    done
}
