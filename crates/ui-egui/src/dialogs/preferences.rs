//! Application preferences and display magnification.

use super::{Dialog, buttons};
use crate::SlideApp;
use crate::theme::{self};
use egui::Ui;
use serde_json::json;

pub(super) fn zoom_dialog(app: &mut SlideApp, ui: &mut Ui, _d: &mut Dialog) -> bool {
    for pct in [400, 200, 150, 100, 75, 66, 50, 33] {
        if ui.radio(false, format!("{pct}%")).clicked() {
            let _ = app.run("view.zoom", json!({"percent": pct}));
        }
    }
    if ui.button("Fit").clicked() {
        app.ui.zoom = None;
    }
    let (ok, cancel) = buttons(ui, "OK");
    ok || cancel
}

pub(super) fn preferences_dialog(app: &mut SlideApp, ui: &mut Ui, _d: &mut Dialog) -> bool {
    ui.label(egui::RichText::new("General").font(theme::bold(13.0)));
    let mut dark = app.ui.brightness == theme::Brightness::Dark;
    if ui.checkbox(&mut dark, "Dark appearance").changed() {
        let _ = app.run("view.dark", json!({"on": dark}));
    }
    let mut scale = app.ui.ui_scale;
    if ui.add(egui::Slider::new(&mut scale, 0.75..=1.75).text("Interface size")).changed() {
        app.ui.ui_scale = scale;
    }
    ui.label(egui::RichText::new("Editing").font(theme::bold(13.0)));
    ui.checkbox(&mut app.session.prefs.smart_guides, "Show smart guides");
    ui.checkbox(&mut app.session.prefs.snap_to_grid, "Snap objects to grid");
    ui.checkbox(&mut app.session.prefs.smart_quotes, "Replace straight quotes with smart quotes");
    ui.checkbox(&mut app.session.prefs.autocorrect, "AutoCorrect as you type");
    ui.horizontal(|ui| {
        ui.label("Your name (comments):");
        ui.text_edit_singleline(&mut app.session.prefs.author);
    });
    let (ok, _) = buttons(ui, "Done");
    ok
}
