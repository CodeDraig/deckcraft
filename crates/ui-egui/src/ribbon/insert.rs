//! Content insertion ribbon controls.

use super::home::layout_menu;
use super::shapes::{paint_preset, shapes_gallery};
use super::{big, enabled};
use crate::SlideApp;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{big_button, big_button_ex, group};
use egui::{CornerRadius, Rect, Sense, Stroke, Ui, vec2};
use serde_json::json;

pub(super) fn insert(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        let (main, arrow) = big_button_ex(ui, Icon::NewSlide, "New\nSlide", enabled(app, "slide.new"), true, false);
        if main.clicked() {
            let _ = app.run("slide.new", json!({}));
        }
        if let Some(a) = arrow {
            egui::Popup::menu(&a).width(420.0).show(|ui| layout_menu(app, ui, "slide.new"));
        }
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::Table, "Table", enabled(app, "insert.table"));
        egui::Popup::menu(&r).show(|ui| {
            if let Some((rr, cc)) = table_grid_picker(ui) {
                let _ = app.run("insert.table", json!({"rows": rr, "cols": cc}));
                ui.close();
            }
            ui.separator();
            if ui.button("Insert Table…").clicked() {
                app.dialog = Some(crate::dialogs::Dialog::new("table"));
                ui.close();
            }
        });
    });
    group(ui, |ui| {
        big(app, ui, Icon::Picture, "Pictures", "app.insertPictureDialog", json!({}));
        let r = big_button(ui, Icon::Screenshot, "Screenshot", false);
        let _ = r.on_hover_text("Screenshot (not available yet)");
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::Shapes, "Shapes", enabled(app, "shape.insert"));
        egui::Popup::menu(&r).show(|ui| {
            if let Some(p) = shapes_gallery(ui) {
                app.session.set_tool(deckcraft_engine::ToolKind::Shape { preset: p.to_string() });
            }
        });
        let r = big_button(ui, Icon::Icons, "Icons", enabled(app, "shape.insert"));
        egui::Popup::menu(&r).width(260.0).show(|ui| icon_library(app, ui));
        big(app, ui, Icon::SmartArt, "SmartArt", "app.dialog", json!({"id": "smartart"}));
        let r = big_button(ui, Icon::Chart, "Chart", enabled(app, "insert.chart"));
        egui::Popup::menu(&r).show(|ui| {
            for k in deckcraft_model::chart::ChartType::MAIN {
                if ui.button(k.label()).clicked() {
                    let _ = app.run("insert.chart", json!({"type": format!("{k:?}")}));
                }
            }
        });
    });
    group(ui, |ui| {
        big(app, ui, Icon::Link, "Link", "app.dialog", json!({"id": "hyperlink"}));
        let r = big_button(ui, Icon::Action, "Action", enabled(app, "insert.actionButton"));
        egui::Popup::menu(&r).show(|ui| {
            for k in ["back", "forward", "beginning", "end", "home", "information", "return", "movie", "document", "sound", "help", "custom"] {
                if ui.button(format!("Action Button: {k}")).clicked() {
                    let _ = app.run("insert.actionButton", json!({"kind": k}));
                }
            }
        });
    });
    group(ui, |ui| {
        big(app, ui, Icon::Comment, "Comment", "app.dialog", json!({"id": "comment"}));
    });
    group(ui, |ui| {
        big(app, ui, Icon::TextBox, "Text\nBox", "insert.textBox", json!({}));
        big(app, ui, Icon::HeaderFooter, "Header &\nFooter", "app.dialog", json!({"id": "headerFooter"}));
        let r = big_button(ui, Icon::WordArt, "WordArt", enabled(app, "insert.wordArt"));
        egui::Popup::menu(&r).show(|ui| {
            for i in 0..6 {
                if ui.button(format!("WordArt style {}", i + 1)).clicked() {
                    let _ = app.run("insert.wordArt", json!({"style": i}));
                }
            }
        });
        big(app, ui, Icon::DateTime, "Date &\nTime", "insert.dateTime", json!({}));
        big(app, ui, Icon::SlideNumber, "Slide\nNumber", "insert.slideNumber", json!({}));
    });
    group(ui, |ui| {
        big(app, ui, Icon::Equation, "Equation", "app.dialog", json!({"id": "equation"}));
        big(app, ui, Icon::Symbol, "Symbol", "app.dialog", json!({"id": "symbol"}));
    });
    group(ui, |ui| {
        big(app, ui, Icon::Video, "Video", "app.insertVideoDialog", json!({}));
        big(app, ui, Icon::Audio, "Audio", "app.insertAudioDialog", json!({}));
    });
}

/// Hover-to-size table grid. Returns (rows, cols) when clicked.
pub fn table_grid_picker(ui: &mut Ui) -> Option<(usize, usize)> {
    let t = Tokens::get(ui.ctx());
    let id = ui.id().with("tgrid");
    let mut hover: (usize, usize) = ui.data_mut(|d| d.get_temp(id).unwrap_or((0, 0)));
    let cell = 16.0;
    let label = if hover.0 > 0 { format!("{}x{} Table", hover.1, hover.0) } else { "Insert Table".into() };
    ui.label(label);
    let (rect, resp) = ui.allocate_exact_size(vec2(10.0 * (cell + 2.0), 8.0 * (cell + 2.0)), Sense::click());
    if let Some(p) = resp.hover_pos() {
        let c = ((p.x - rect.min.x) / (cell + 2.0)).floor() as usize + 1;
        let r = ((p.y - rect.min.y) / (cell + 2.0)).floor() as usize + 1;
        hover = (r.min(8), c.min(10));
    }
    for r in 0..8 {
        for c in 0..10 {
            let cr = Rect::from_min_size(rect.min + vec2(c as f32 * (cell + 2.0), r as f32 * (cell + 2.0)), vec2(cell, cell));
            let on = r < hover.0 && c < hover.1;
            ui.painter().rect(
                cr,
                CornerRadius::ZERO,
                if on { t.accent.gamma_multiply(0.3) } else { t.card },
                Stroke::new(1.0, if on { t.accent } else { t.border }),
                egui::StrokeKind::Inside,
            );
        }
    }
    ui.data_mut(|d| d.insert_temp(id, hover));
    if resp.clicked() && hover.0 > 0 { Some(hover) } else { None }
}

/// A small built-in icon library: our own simple pictograms built from preset shapes.
pub(super) fn icon_library(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let items: [(&str, &str); 24] = [
        ("star5", "Star"),
        ("heart", "Heart"),
        ("lightningBolt", "Lightning"),
        ("sun", "Sun"),
        ("moon", "Moon"),
        ("cloud", "Cloud"),
        ("smileyFace", "Smile"),
        ("noSmoking", "Not allowed"),
        ("mathPlus", "Plus"),
        ("mathMultiply", "Multiply"),
        ("mathEqual", "Equals"),
        ("chevron", "Chevron"),
        ("rightArrow", "Arrow"),
        ("quadArrow", "Move"),
        ("uturnArrow", "Return"),
        ("flowChartDecision", "Decision"),
        ("flowChartMagneticDisk", "Database"),
        ("flowChartDocument", "Document"),
        ("wedgeRoundRectCallout", "Speech"),
        ("cloudCallout", "Thought"),
        ("teardrop", "Pin"),
        ("donut", "Ring"),
        ("irregularSeal1", "Burst"),
        ("ribbon2", "Banner"),
    ];
    ui.horizontal_wrapped(|ui| {
        for (preset, label) in items {
            let (r, resp) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(4), t.hover);
            }
            paint_preset(ui.painter(), r.shrink(6.0), preset, t.text, t.text);
            if resp.on_hover_text(label).clicked() {
                let size = app.session.active().map(|d| d.doc.slide_size).unwrap_or(deckcraft_model::defaults::WIDE);
                let _ = app.run("shape.insert", json!({"preset": preset, "rect": [size.width / 2.0 - 48.0, size.height / 2.0 - 48.0, 96, 96]}));
                let _ = app.run("shape.fill", json!({"color": "tx1"}));
                let _ = app.run("shape.line", json!({"none": true}));
                ui.close();
            }
        }
    });
}
