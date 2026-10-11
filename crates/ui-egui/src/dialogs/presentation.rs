//! Presentation playback configuration forms.

use super::{Dialog, buttons, check, field};
use crate::SlideApp;
use egui::Ui;
use serde_json::json;

pub(super) fn setup_show_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let sh = app.session.active().map(|s| s.doc.show.clone()).unwrap_or_default();
    let ty = d.get("type", &sh.show_type);
    ui.label("Show type");
    for (l, v) in [
        ("Presented by a speaker (full screen)", "speaker"),
        ("Browsed by an individual (window)", "browsed"),
        ("Browsed at a kiosk (full screen)", "kiosk"),
    ] {
        if ui.radio(ty == v, l).clicked() {
            d.fields.insert("type".into(), v.into());
        }
    }
    let lp = check(ui, d, "Loop continuously until 'Esc'", "loop", sh.loop_until_esc);
    let nn = check(ui, d, "Show without narration", "nn", sh.without_narration);
    let na = check(ui, d, "Show without animation", "na", sh.without_animation);
    let ut = check(ui, d, "Use timings, if present", "ut", sh.use_timings);
    let (ok, cancel) = buttons(ui, "OK");
    if ok {
        let ty = d.get("type", "speaker");
        let _ = app.run("show.setup", json!({"type": ty, "loop": lp, "noNarration": nn, "noAnimation": na, "useTimings": ut}));
    }
    ok || cancel
}

pub(super) fn custom_show_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let shows = app.session.active().map(|s| s.doc.custom_shows.clone()).unwrap_or_default();
    for s in &shows {
        ui.horizontal(|ui| {
            ui.label(format!("{} ({} slides)", s.name, s.slides.len()));
            if ui.small_button("Delete").clicked() {
                let _ = app.run("show.customShow", json!({"name": s.name, "delete": true}));
            }
        });
    }
    field(ui, d, "New show name:", "name", "Custom Show 1");
    field(ui, d, "Slides (e.g. 1,3,5):", "slides", "1");
    let (ok, cancel) = buttons(ui, "Create");
    if ok {
        let idx: Vec<usize> =
            d.get("slides", "").split(',').filter_map(|s| s.trim().parse::<usize>().ok()).filter(|v| *v > 0).map(|v| v - 1).collect();
        let _ = app.run("show.customShow", json!({"name": d.get("name", "Custom Show"), "slides": idx}));
    }
    ok || cancel
}
