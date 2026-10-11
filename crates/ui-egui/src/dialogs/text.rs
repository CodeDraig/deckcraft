//! Text and text-metadata editing forms.

use super::{Dialog, buttons, check, field, num};
use crate::SlideApp;
use egui::Ui;
use serde_json::json;

pub(super) fn hyperlink_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let mode = d.get("mode", "url");
    ui.horizontal(|ui| {
        if ui.selectable_label(mode == "url", "Web Page or File").clicked() {
            d.fields.insert("mode".into(), "url".into());
        }
        if ui.selectable_label(mode == "slide", "This Document").clicked() {
            d.fields.insert("mode".into(), "slide".into());
        }
        if ui.selectable_label(mode == "email", "Email Address").clicked() {
            d.fields.insert("mode".into(), "email".into());
        }
    });
    match mode.as_str() {
        "slide" => {
            let titles: Vec<String> = app
                .session
                .active()
                .map(|s| s.doc.slides.iter().enumerate().map(|(i, sl)| format!("{}. {}", i + 1, sl.title())).collect())
                .unwrap_or_default();
            let sel = d.get("slide", "0").parse::<usize>().unwrap_or(0);
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                for (i, t) in titles.iter().enumerate() {
                    if ui.selectable_label(sel == i, t).clicked() {
                        d.fields.insert("slide".into(), i.to_string());
                    }
                }
            });
        }
        "email" => field(ui, d, "Email address:", "email", ""),
        _ => field(ui, d, "Address:", "url", "https://"),
    }
    field(ui, d, "ScreenTip:", "tip", "");
    let (ok, cancel) = buttons(ui, "OK");
    if ok {
        let tip = d.get("tip", "");
        let p = match mode.as_str() {
            "slide" => json!({"slide": d.get("slide", "0").parse::<usize>().unwrap_or(0), "tooltip": tip}),
            "email" => json!({"url": format!("mailto:{}", d.get("email", "")), "tooltip": tip}),
            _ => json!({"url": d.get("url", ""), "tooltip": tip}),
        };
        let _ = app.run("insert.hyperlink", p);
    }
    ok || cancel
}

pub(super) fn alt_text_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let cur = app.session.active().and_then(|s| s.selected_shapes().first().map(|x| x.descr.clone())).unwrap_or_default();
    ui.label("How would you describe this object and its context to someone who is blind or has low vision?");
    let mut v = d.get("text", &cur);
    ui.add(egui::TextEdit::multiline(&mut v).desired_rows(4).desired_width(340.0));
    d.fields.insert("text".into(), v.clone());
    let deco = check(ui, d, "Mark as decorative", "deco", false);
    let (ok, cancel) = buttons(ui, "OK");
    if ok {
        let _ = app.run("shape.altText", json!({"text": v, "decorative": deco}));
    }
    ok || cancel
}

pub(super) fn paragraph_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    field(ui, d, "Before text (in):", "left", "0");
    field(ui, d, "Special — hanging (in):", "hang", "0");
    field(ui, d, "Spacing before (pt):", "before", "0");
    field(ui, d, "Spacing after (pt):", "after", "0");
    field(ui, d, "Line spacing (lines):", "lines", "1.0");
    let (ok, cancel) = buttons(ui, "OK");
    if ok {
        let _ = app.run("format.paragraph", json!({"indentLeft": num(d, "left", 0.0) * 72.0, "indentFirst": -num(d, "hang", 0.0) * 72.0, "spaceBefore": num(d, "before", 0.0), "spaceAfter": num(d, "after", 0.0), "lineSpacing": num(d, "lines", 1.0)}));
    }
    ok || cancel
}

pub(super) fn language_dialog(_app: &mut SlideApp, ui: &mut Ui, _d: &mut Dialog) -> bool {
    ui.label("Mark selected text as:");
    for l in [
        "English (United States)",
        "English (United Kingdom)",
        "Español",
        "Français",
        "Deutsch",
        "Italiano",
        "Português",
        "日本語",
        "中文",
        "العربية",
    ] {
        let _ = ui.selectable_label(l.starts_with("English (United States)"), l);
    }
    let (ok, cancel) = buttons(ui, "OK");
    ok || cancel
}
