//! Slide dimensions and repeated header/footer presentation settings.

use super::{Dialog, buttons, check, field, num};
use crate::SlideApp;
use egui::Ui;
use serde_json::json;

pub(super) fn slide_size_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let cur = app.session.active().map(|s| s.doc.slide_size).unwrap_or(deckcraft_model::defaults::WIDE);
    ui.label("Slides sized for:");
    let preset = d.get("preset", "");
    egui::ComboBox::from_id_salt("sizes").selected_text(if preset.is_empty() { "Custom" } else { &preset }).show_ui(ui, |ui| {
        for (label, w, h) in deckcraft_model::defaults::SLIDE_SIZES {
            if ui.selectable_label(preset == *label, *label).clicked() {
                d.fields.insert("preset".into(), label.to_string());
                d.fields.insert("w".into(), format!("{:.2}", w / 72.0));
                d.fields.insert("h".into(), format!("{:.2}", h / 72.0));
            }
        }
    });
    field(ui, d, "Width (in):", "w", &format!("{:.2}", cur.width / 72.0));
    field(ui, d, "Height (in):", "h", &format!("{:.2}", cur.height / 72.0));
    let maximize = check(ui, d, "Maximize content (else ensure fit)", "max", false);
    let (ok, cancel) = buttons(ui, "OK");
    if ok {
        let (w, h) = (num(d, "w", cur.width / 72.0) * 72.0, num(d, "h", cur.height / 72.0) * 72.0);
        let _ = app.run("design.slideSize", json!({"w": w, "h": h, "scale": if maximize { "maximize" } else { "ensureFit" }}));
    }
    ok || cancel
}

pub(super) fn header_footer_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let hf = app.session.active().map(|s| s.doc.header_footer.clone()).unwrap_or_default();
    let date = check(ui, d, "Date and time", "date", hf.date);
    field(ui, d, "  Fixed date (empty = update automatically):", "dateText", &hf.date_text);
    let num_on = check(ui, d, "Slide number", "num", hf.slide_number);
    let foot = check(ui, d, "Footer", "footer", hf.footer);
    field(ui, d, "  Footer text:", "footerText", &hf.footer_text);
    let hide = check(ui, d, "Don't show on title slide", "hide", hf.hide_on_title);
    ui.add_space(6.0);
    let mut apply_all = false;
    let mut apply = false;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        apply_all = ui.button("Apply to All").clicked();
        apply = ui.button("Apply").clicked();
        if ui.button("Cancel").clicked() {
            d.fields.insert("cancel".into(), "1".into());
        }
    });
    if apply || apply_all {
        let dt = d.get("dateText", "");
        let ft = d.get("footerText", "");
        let _ = app.run(
            "design.headerFooter",
            json!({"date": date, "dateText": dt, "slideNumber": num_on, "footer": foot, "footerText": ft, "hideOnTitle": hide, "all": apply_all}),
        );
    }
    apply || apply_all || d.fields.contains_key("cancel")
}
