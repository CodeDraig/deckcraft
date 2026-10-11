//! Media trim and fade form.

use super::{Dialog, buttons, field, num};
use crate::SlideApp;
use egui::Ui;
use serde_json::json;

pub(super) fn trim_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    field(ui, d, "Start time (s):", "start", "0");
    field(ui, d, "End trim (s):", "end", "0");
    field(ui, d, "Fade in (s):", "fin", "0");
    field(ui, d, "Fade out (s):", "fout", "0");
    let (ok, cancel) = buttons(ui, "OK");
    if ok {
        let _ = app.run("media.options", json!({"trimStart": (num(d, "start", 0.0) * 1000.0) as u64, "trimEnd": (num(d, "end", 0.0) * 1000.0) as u64, "fadeIn": (num(d, "fin", 0.0) * 1000.0) as u64, "fadeOut": (num(d, "fout", 0.0) * 1000.0) as u64}));
    }
    ok || cancel
}
