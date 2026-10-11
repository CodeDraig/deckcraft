//! Clipboard ribbon group.

use super::{enabled, small};
use crate::SlideApp;
use crate::icons::Icon;
use crate::widgets::{big_button_ex, group, rows, small_button};
use egui::Ui;
use serde_json::json;

pub(super) fn clipboard_group(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        let (main, arrow) = big_button_ex(ui, Icon::Paste, "Paste", enabled(app, "edit.paste"), true, false);
        if main.on_hover_text("Paste (⌘V)").clicked() {
            let _ = app.run("edit.paste", json!({}));
        }
        if let Some(a) = arrow {
            egui::Popup::menu(&a).show(|ui| {
                if ui.button("Paste").clicked() {
                    let _ = app.run("edit.paste", json!({}));
                }
                if ui.button("Paste and Match Formatting").clicked() {
                    let _ = app.run("edit.pasteText", json!({}));
                }
                if ui.button("Paste Formatting").clicked() {
                    let _ = app.run("format.painterApply", json!({}));
                }
            });
        }
        rows(ui, |ui| {
            small(app, ui, Icon::Cut, "Cut", "edit.cut", json!({}));
            small(app, ui, Icon::Copy, "Copy", "edit.copy", json!({}));
            let on = app.session.painter.is_some();
            let en = enabled(app, "format.painter");
            let r = small_button(ui, Icon::FormatPainter, "Format", en, on).on_hover_text("Format Painter (double-click to keep it on)");
            if r.double_clicked() {
                let _ = app.run("format.painter", json!({"sticky": true}));
            } else if r.clicked() {
                if on {
                    app.session.painter = None;
                } else {
                    let _ = app.run("format.painter", json!({}));
                }
            }
        });
    });
}
