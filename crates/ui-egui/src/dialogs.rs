//! Dialogs (Insert Table, Slide Size, Header & Footer, Hyperlink, Set Up Show, Zoom, …), the start
//! screen and the About window.

mod welcome;
pub use welcome::start_screen;

mod rename;
use rename::rename_dialog;

mod about;
use about::about_dialog;
pub use about::{about, paint_logo, version_line};

mod preferences;
use preferences::{preferences_dialog, zoom_dialog};

mod quit;
use quit::quit_dialog;

mod export;
use export::export_dialog;

mod media;
use media::trim_dialog;

mod chart;
use chart::chart_data_dialog;

mod review;
use review::{accessibility_dialog, comment_dialog, spelling_dialog};

mod insertion;
use insertion::{equation_dialog, outline_dialog, smartart_dialog, symbol_dialog, table_dialog};

mod presentation;
use presentation::{custom_show_dialog, setup_show_dialog};

mod text;
use text::{alt_text_dialog, hyperlink_dialog, language_dialog, paragraph_dialog};

mod document;
use document::{header_footer_dialog, slide_size_dialog};

use egui::{Align2, Ui, vec2};
use serde_json::Value;

use crate::SlideApp;
use crate::theme::Tokens;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Dialog {
    pub id: String,
    pub params: Value,
    /// Field values while the dialog is open.
    #[serde(skip)]
    pub fields: std::collections::HashMap<String, String>,
}

impl Dialog {
    pub fn new(id: &str) -> Self {
        Dialog { id: id.into(), params: Value::Null, fields: Default::default() }
    }
    fn get(&mut self, k: &str, default: &str) -> String {
        self.fields.entry(k.into()).or_insert_with(|| default.to_string()).clone()
    }
}

fn title(id: &str) -> &'static str {
    match id {
        "table" => "Insert Table",
        "slideSize" => "Slide Size",
        "headerFooter" => "Header and Footer",
        "hyperlink" => "Insert Hyperlink",
        "setupShow" => "Set Up Slide Show",
        "zoom" => "Zoom",
        "comment" => "New Comment",
        "altText" => "Alt Text",
        "outline" => "Slides from Outline",
        "renameSection" => "Rename Section",
        "renameShape" => "Rename",
        "renameLayout" => "Rename Layout",
        "paragraph" => "Paragraph",
        "about" => "About DeckCraft",
        "preferences" => "Preferences",
        "export" => "Export",
        "symbol" => "Symbol",
        "equation" => "Equation",
        "spelling" => "Spelling",
        "accessibility" => "Accessibility Checker",
        "language" => "Language",
        "customShow" => "Custom Shows",
        "chartData" => "Chart Data",
        "trim" => "Trim Media",
        "smartart" => "Choose a SmartArt Graphic",
        "quit" => "DeckCraft",
        _ => "DeckCraft",
    }
}

pub fn show(app: &mut SlideApp, ctx: &egui::Context) {
    let Some(mut d) = app.dialog.take() else { return };
    let mut open = true;
    let mut close = false;
    egui::Window::new(title(&d.id))
        .id(egui::Id::new(("dialog", d.id.clone())))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, -40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            close = body(app, ui, &mut d);
        });
    if open && !close && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.dialog = Some(d);
    }
}

fn buttons(ui: &mut Ui, ok_label: &str) -> (bool, bool) {
    let mut ok = false;
    let mut cancel = false;
    ui.add_space(8.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let t = Tokens::get(ui.ctx());
        if ui.add(egui::Button::new(egui::RichText::new(ok_label).color(t.accent_text)).fill(t.accent).min_size(vec2(72.0, 24.0))).clicked()
            || ui.input(|i| i.key_pressed(egui::Key::Enter))
        {
            ok = true;
        }
        if ui.add(egui::Button::new("Cancel").min_size(vec2(72.0, 24.0))).clicked() {
            cancel = true;
        }
    });
    (ok, cancel)
}

fn field(ui: &mut Ui, d: &mut Dialog, label: &str, key: &str, default: &str) {
    let mut v = d.get(key, default);
    ui.horizontal(|ui| {
        ui.add_sized(vec2(120.0, 20.0), egui::Label::new(label));
        ui.add(egui::TextEdit::singleline(&mut v).desired_width(180.0));
    });
    d.fields.insert(key.into(), v);
}

fn check(ui: &mut Ui, d: &mut Dialog, label: &str, key: &str, default: bool) -> bool {
    let mut v = d.get(key, if default { "1" } else { "0" }) == "1";
    ui.checkbox(&mut v, label);
    d.fields.insert(key.into(), if v { "1".into() } else { "0".into() });
    v
}

fn num(d: &mut Dialog, key: &str, default: f64) -> f64 {
    d.get(key, "").trim().trim_end_matches(['"', '%', ' ']).parse::<f64>().ok().filter(|v| v.is_finite()).unwrap_or(default)
}

/// Returns true when the dialog should close.
fn body(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    match d.id.as_str() {
        "table" => table_dialog(app, ui, d),
        "slideSize" => slide_size_dialog(app, ui, d),
        "headerFooter" => header_footer_dialog(app, ui, d),
        "hyperlink" => hyperlink_dialog(app, ui, d),
        "setupShow" => setup_show_dialog(app, ui, d),
        "quit" => quit_dialog(app, ui, d),
        "zoom" => zoom_dialog(app, ui, d),
        "comment" => comment_dialog(app, ui, d),
        "altText" => alt_text_dialog(app, ui, d),
        "outline" => outline_dialog(app, ui, d),
        "renameSection" | "renameShape" | "renameLayout" => rename_dialog(app, ui, d),
        "paragraph" => paragraph_dialog(app, ui, d),
        "export" => export_dialog(app, ui, d),
        "symbol" => symbol_dialog(app, ui, d),
        "equation" => equation_dialog(app, ui, d),
        "spelling" => spelling_dialog(app, ui, d),
        "accessibility" => accessibility_dialog(app, ui, d),
        "language" => language_dialog(app, ui, d),
        "customShow" => custom_show_dialog(app, ui, d),
        "chartData" => chart_data_dialog(app, ui, d),
        "trim" => trim_dialog(app, ui, d),
        "smartart" => smartart_dialog(app, ui, d),
        "about" => about_dialog(app, ui, d),
        "preferences" => preferences_dialog(app, ui, d),
        _ => {
            ui.label(format!("“{}” is not available yet.", d.id));
            let (ok, cancel) = buttons(ui, "OK");
            ok || cancel
        }
    }
}
