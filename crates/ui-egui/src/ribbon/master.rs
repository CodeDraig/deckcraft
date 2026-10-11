//! Master and layout editing controls.

use super::big;
use crate::SlideApp;
use crate::icons::Icon;
use crate::widgets::{big_button, group};
use egui::Ui;
use serde_json::json;

pub(super) fn slide_master(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        big(app, ui, Icon::NewSlide, "Insert\nLayout", "master.insertLayout", json!({}));
        big(app, ui, Icon::DeleteSlide, "Delete", "master.deleteLayout", json!({}));
        big(app, ui, Icon::Layout, "Rename", "app.dialog", json!({"id": "renameLayout"}));
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::Table, "Insert\nPlaceholder", true);
        egui::Popup::menu(&r).show(|ui| {
            for (l, k) in
                [("Content", "content"), ("Text", "text"), ("Picture", "picture"), ("Chart", "chart"), ("Table", "table"), ("Media", "media")]
            {
                if ui.button(l).clicked() {
                    let _ = app.run("master.insertPlaceholder", json!({"kind": k}));
                }
            }
        });
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::Themes, "Themes", true);
        egui::Popup::menu(&r).show(|ui| {
            for th in deckcraft_model::theme::builtin_themes() {
                if ui.button(&th.name).clicked() {
                    let _ = app.run("design.theme", json!({"name": th.name}));
                }
            }
        });
        big(app, ui, Icon::FormatBackground, "Background\nStyles", "view.pane", json!({"pane": "background"}));
    });
    group(ui, |ui| {
        if big_button(ui, Icon::Close, "Close\nMaster View", true).clicked() {
            let _ = app.run("view.closeMaster", json!({}));
        }
    });
}
