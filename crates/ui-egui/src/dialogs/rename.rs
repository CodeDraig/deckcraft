//! Named document-object rename form.

use super::{Dialog, buttons, field};
use crate::SlideApp;
use egui::Ui;
use serde_json::{Value, json};

pub(super) fn rename_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let cur = d.params.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    field(ui, d, "Name:", "name", &cur);
    let (ok, cancel) = buttons(ui, "Rename");
    if ok {
        let name = d.get("name", "");
        match d.id.as_str() {
            "renameSection" => {
                let _ = app.run("section.rename", json!({"index": d.params.get("index").cloned().unwrap_or_default(), "name": name}));
            }
            "renameShape" => {
                let _ = app.run("shape.rename", json!({"id": d.params.get("id").cloned().unwrap_or_default(), "name": name}));
            }
            _ => {
                let _ = app.run("master.renameLayout", json!({"name": name}));
            }
        }
    }
    ok || cancel
}
