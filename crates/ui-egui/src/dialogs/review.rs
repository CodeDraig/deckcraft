//! Comments and document review forms.

use super::{Dialog, buttons};
use crate::SlideApp;
use egui::Ui;
use serde_json::{Value, json};

pub(super) fn comment_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let mut v = d.get("text", "");
    ui.add(egui::TextEdit::multiline(&mut v).hint_text("Start a conversation").desired_rows(4).desired_width(320.0));
    d.fields.insert("text".into(), v.clone());
    let (ok, cancel) = buttons(ui, "Post");
    if ok && !v.trim().is_empty() {
        let _ = app.run("comment.add", json!({"text": v}));
        app.ui.pane = Some("comments".into());
    }
    ok || cancel
}

pub(super) fn spelling_dialog(app: &mut SlideApp, ui: &mut Ui, _d: &mut Dialog) -> bool {
    let v = app.session.execute("review.spelling", &json!({})).unwrap_or_default();
    let list = v.as_array().cloned().unwrap_or_default();
    if list.is_empty() {
        ui.label("The spelling check is complete. No issues found.");
    }
    for item in &list {
        ui.horizontal(|ui| {
            let w = item.get("word").and_then(Value::as_str).unwrap_or("");
            ui.label(format!("Slide {}: “{w}”", item.get("slide").and_then(Value::as_u64).unwrap_or(0) + 1));
            if ui.small_button("Go to").clicked() {
                let _ = app.run("slide.go", json!({"index": item.get("slide").cloned().unwrap_or_default()}));
            }
        });
    }
    let (ok, cancel) = buttons(ui, "Close");
    ok || cancel
}

pub(super) fn accessibility_dialog(app: &mut SlideApp, ui: &mut Ui, _d: &mut Dialog) -> bool {
    let v = app.session.execute("review.accessibility", &json!({})).unwrap_or_default();
    let list = v.as_array().cloned().unwrap_or_default();
    if list.is_empty() {
        ui.label("No accessibility issues found. People with disabilities should not have difficulty reading this document.");
    }
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for item in &list {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Slide {}: {}{}",
                    item.get("slide").and_then(Value::as_u64).unwrap_or(0) + 1,
                    item.get("issue").and_then(Value::as_str).unwrap_or(""),
                    item.get("name").and_then(Value::as_str).map(|n| format!(" — {n}")).unwrap_or_default()
                ));
                if ui.small_button("Go to").clicked() {
                    let _ = app.run("slide.go", json!({"index": item.get("slide").cloned().unwrap_or_default()}));
                    if let Some(id) = item.get("shape") {
                        let _ = app.run("edit.select", json!({"ids": [id]}));
                    }
                }
            });
        }
    });
    let (ok, cancel) = buttons(ui, "Close");
    ok || cancel
}
