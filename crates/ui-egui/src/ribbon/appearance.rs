//! Shape appearance controls and presets.

use super::shapes::shapes_gallery;
use super::{arrange_group, arrange_menu, big, cref_param, enabled, scheme, size_group};
use crate::SlideApp;
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::widgets::{self, big_button, group, rows, small_button, small_button_ex};
use egui::{Align2, Color32, CornerRadius, Sense, Stroke, Ui, vec2};
use serde_json::json;

pub(super) fn drawing_group(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        let en = enabled(app, "shape.insert");
        let r = big_button(ui, Icon::Shapes, "Shapes", en).on_hover_text("Shapes");
        egui::Popup::menu(&r).show(|ui| {
            if let Some(p) = shapes_gallery(ui) {
                app.session.set_tool(deckcraft_engine::ToolKind::Shape { preset: p.to_string() });
            }
        });
        big(app, ui, Icon::TextBox, "Text Box", "insert.textBox", json!({}));
        let sel = enabled(app, "shape.fill");
        let r = big_button(ui, Icon::Arrange, "Arrange", sel);
        egui::Popup::menu(&r).show(|ui| arrange_menu(app, ui));
        let r = big_button(ui, Icon::QuickStyles, "Quick\nStyles", sel);
        egui::Popup::menu(&r).width(330.0).show(|ui| quick_styles(app, ui));
        rows(ui, |ui| {
            let sc = scheme(app);
            let (m, a) = small_button_ex(ui, Icon::ShapeFill, "Shape Fill", sel, false, true);
            if m.clicked() {
                let _ = app.run("shape.fill", json!({"color": "accent1"}));
            }
            if let Some(a) = a {
                egui::Popup::menu(&a).show(|ui| {
                    if let Some(c) = widgets::color_grid(ui, &sc, Some("No Fill")) {
                        match c {
                            Some(c) => {
                                let _ = app.run("shape.fill", json!({"color": cref_param(&c)}));
                            }
                            None => {
                                let _ = app.run("shape.fill", json!({"none": true}));
                            }
                        }
                    }
                });
            }
            let (m, a) = small_button_ex(ui, Icon::ShapeOutline, "Shape Outline", sel, false, true);
            if m.clicked() {
                let _ = app.run("shape.line", json!({"color": "tx1", "width": 1}));
            }
            if let Some(a) = a {
                egui::Popup::menu(&a).show(|ui| outline_menu(app, ui, &sc));
            }
            let (_, a) = small_button_ex(ui, Icon::ShapeEffects, "Shape Effects", sel, false, true);
            if let Some(a) = a {
                egui::Popup::menu(&a).show(|ui| effects_menu(app, ui));
            }
        });
    });
}

pub fn outline_menu(app: &mut SlideApp, ui: &mut Ui, sc: &deckcraft_color::ColorScheme) {
    if let Some(c) = widgets::color_grid(ui, sc, Some("No Outline")) {
        match c {
            Some(c) => {
                let _ = app.run("shape.line", json!({"color": cref_param(&c)}));
            }
            None => {
                let _ = app.run("shape.line", json!({"none": true}));
            }
        }
    }
    ui.menu_button("Weight", |ui| {
        for w in [0.25, 0.5, 0.75, 1.0, 1.5, 2.25, 3.0, 4.5, 6.0] {
            if ui.button(format!("{w} pt")).clicked() {
                let _ = app.run("shape.line", json!({"width": w}));
            }
        }
    });
    ui.menu_button("Dashes", |ui| {
        for (l, d) in [
            ("Solid", "solid"),
            ("Round Dot", "sysDot"),
            ("Square Dot", "sysDash"),
            ("Dash", "dash"),
            ("Dash Dot", "dashDot"),
            ("Long Dash", "lgDash"),
            ("Long Dash Dot", "lgDashDot"),
        ] {
            if ui.button(l).clicked() {
                let _ = app.run("shape.line", json!({"dash": d}));
            }
        }
    });
    ui.menu_button("Arrows", |ui| {
        for (l, h, tl) in [
            ("None", "none", "none"),
            ("Arrow at end", "none", "triangle"),
            ("Arrow at start", "triangle", "none"),
            ("Arrows at both ends", "triangle", "triangle"),
            ("Stealth end", "none", "stealth"),
            ("Oval ends", "oval", "oval"),
        ] {
            if ui.button(l).clicked() {
                let _ = app.run("shape.line", json!({"head": h, "tail": tl}));
            }
        }
    });
}

pub fn effects_menu(app: &mut SlideApp, ui: &mut Ui) {
    ui.menu_button("Shadow", |ui| {
        for (l, v) in [
            ("No Shadow", "none"),
            ("Outer", "outer"),
            ("Bottom", "bottom"),
            ("Centered", "center"),
            ("Inner", "inner"),
            ("Perspective", "perspective"),
        ] {
            if ui.button(l).clicked() {
                let _ = app.run("shape.effects", json!({"shadow": v}));
            }
        }
    });
    ui.menu_button("Reflection", |ui| {
        for (l, v) in [("No Reflection", "none"), ("Tight", "tight"), ("Half", "half"), ("Full", "full")] {
            if ui.button(l).clicked() {
                let _ = app.run("shape.effects", json!({"reflection": v}));
            }
        }
    });
    ui.menu_button("Glow", |ui| {
        if ui.button("No Glow").clicked() {
            let _ = app.run("shape.effects", json!({"glow": null}));
        }
        for r in [5.0, 8.0, 11.0, 18.0] {
            if ui.button(format!("{r} pt glow")).clicked() {
                let _ = app.run("shape.effects", json!({"glow": r}));
            }
        }
    });
    ui.menu_button("Soft Edges", |ui| {
        for r in [0.0, 1.0, 2.5, 5.0, 10.0, 25.0] {
            if ui.button(if r == 0.0 { "No Soft Edges".to_string() } else { format!("{r} pt") }).clicked() {
                let _ = app.run("shape.effects", json!({"softEdges": r}));
            }
        }
    });
    ui.menu_button("Bevel", |ui| {
        for b in [
            "none",
            "circle",
            "relaxedInset",
            "cross",
            "coolSlant",
            "angle",
            "softRound",
            "convex",
            "slope",
            "divot",
            "riblet",
            "hardEdge",
            "artDeco",
        ] {
            if ui.button(b).clicked() {
                let _ = app.run("shape.effects", json!({"bevel": b}));
            }
        }
    });
    if ui.button("Format Shape…").clicked() {
        let _ = app.run("view.pane", json!({"pane": "format", "tab": "effects"}));
    }
}

pub(super) fn quick_styles(app: &mut SlideApp, ui: &mut Ui) {
    let sc = scheme(app);
    let slots = [
        deckcraft_color::SchemeSlot::Dk1,
        deckcraft_color::SchemeSlot::Accent1,
        deckcraft_color::SchemeSlot::Accent2,
        deckcraft_color::SchemeSlot::Accent3,
        deckcraft_color::SchemeSlot::Accent4,
        deckcraft_color::SchemeSlot::Accent5,
        deckcraft_color::SchemeSlot::Accent6,
    ];
    egui::Grid::new("quickstyles").spacing(vec2(4.0, 4.0)).show(ui, |ui| {
        for row in 0..6 {
            for (k, slot) in slots.iter().enumerate() {
                let c = theme::to_color32(sc.get(*slot));
                let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 26.0), Sense::click());
                let p = ui.painter();
                let (fill, stroke, text) = match row {
                    0 => (Color32::WHITE, c, Color32::BLACK),
                    2 => (c.gamma_multiply(0.35).to_opaque().lerp_to_gamma(Color32::WHITE, 0.6), Color32::TRANSPARENT, c),
                    5 => (c.lerp_to_gamma(Color32::BLACK, 0.25), Color32::WHITE, Color32::WHITE),
                    _ => (c, c.lerp_to_gamma(Color32::BLACK, 0.3), Color32::WHITE),
                };
                p.rect(rect.shrink(2.0), CornerRadius::same(3), fill, Stroke::new(1.5, stroke), egui::StrokeKind::Inside);
                p.text(rect.center(), Align2::CENTER_CENTER, "Abc", theme::font(11.0), text);
                if resp.hovered() {
                    p.rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.5, Tokens::get(ui.ctx()).accent), egui::StrokeKind::Inside);
                }
                if resp.clicked() {
                    let _ = app.run("shape.quickStyle", json!({"index": row * 7 + k}));
                    ui.close();
                }
            }
            ui.end_row();
        }
    });
}

pub(super) fn shape_format(app: &mut SlideApp, ui: &mut Ui) {
    let sc = scheme(app);
    let sel = enabled(app, "shape.fill");
    group(ui, |ui| {
        let r = big_button(ui, Icon::Shapes, "Shapes", true);
        egui::Popup::menu(&r).show(|ui| {
            if let Some(p) = shapes_gallery(ui) {
                app.session.set_tool(deckcraft_engine::ToolKind::Shape { preset: p.to_string() });
            }
        });
        big(app, ui, Icon::TextBox, "Text\nBox", "insert.textBox", json!({}));
        let r = big_button(ui, Icon::EditShape, "Edit\nShape", sel);
        egui::Popup::menu(&r).show(|ui| {
            ui.label("Change Shape");
            if let Some(p) = shapes_gallery(ui) {
                let _ = app.run("shape.change", json!({"preset": p}));
            }
        });
        let can_merge = enabled(app, "shape.merge");
        let r = big_button(ui, Icon::MergeShapes, "Merge\nShapes", can_merge).on_hover_text("Merge Shapes");
        egui::Popup::menu(&r).show(|ui| {
            for (op, label) in deckcraft_engine::cmd::merge::OPS {
                if ui.button(label).clicked() {
                    let _ = app.run("shape.merge", json!({"op": op}));
                    ui.close();
                }
            }
        });
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::QuickStyles, "Shape\nStyles", sel);
        egui::Popup::menu(&r).width(330.0).show(|ui| quick_styles(app, ui));
        rows(ui, |ui| {
            let (m, a) = small_button_ex(ui, Icon::ShapeFill, "Shape Fill", sel, false, true);
            if m.clicked() {
                let _ = app.run("shape.fill", json!({"color": "accent1"}));
            }
            if let Some(a) = a {
                egui::Popup::menu(&a).show(|ui| {
                    if let Some(c) = widgets::color_grid(ui, &sc, Some("No Fill")) {
                        match c {
                            Some(c) => {
                                let _ = app.run("shape.fill", json!({"color": cref_param(&c)}));
                            }
                            None => {
                                let _ = app.run("shape.fill", json!({"none": true}));
                            }
                        }
                    }
                    if ui.button("Gradient…").clicked() {
                        let _ = app.run("view.pane", json!({"pane": "format", "tab": "fill"}));
                    }
                });
            }
            let (_, a) = small_button_ex(ui, Icon::ShapeOutline, "Shape Outline", sel, false, true);
            if let Some(a) = a {
                egui::Popup::menu(&a).show(|ui| outline_menu(app, ui, &sc));
            }
            let (_, a) = small_button_ex(ui, Icon::ShapeEffects, "Shape Effects", sel, false, true);
            if let Some(a) = a {
                egui::Popup::menu(&a).show(|ui| effects_menu(app, ui));
            }
        });
    });
    group(ui, |ui| {
        rows(ui, |ui| {
            let (_, a) = small_button_ex(ui, Icon::TextFill, "Text Fill", sel, false, true);
            if let Some(a) = a {
                egui::Popup::menu(&a).show(|ui| {
                    if let Some(Some(c)) = widgets::color_grid(ui, &sc, None) {
                        let _ = app.run("format.color", json!({"color": cref_param(&c)}));
                    }
                });
            }
            let (_, a) = small_button_ex(ui, Icon::TextOutline, "Text Outline", sel, false, true);
            if let Some(a) = a {
                egui::Popup::menu(&a).show(|ui| {
                    if let Some(c) = widgets::color_grid(ui, &sc, Some("No Outline")) {
                        let _ = app.run("format.textOutline", json!({"color": c.map(|c| cref_param(&c))}));
                    }
                });
            }
            let r = small_button(ui, Icon::TextEffects, "Text Effects", sel, false);
            egui::Popup::menu(&r).show(|ui| {
                if ui.button("Shadow").clicked() {
                    let _ = app.run("format.textShadow", json!({}));
                }
            });
        });
    });
    group(ui, |ui| {
        big(app, ui, Icon::AltText, "Alt\nText", "app.dialog", json!({"id": "altText"}));
    });
    arrange_group(app, ui);
    size_group(app, ui);
    group(ui, |ui| {
        big(app, ui, Icon::FormatBackground, "Format\nPane", "view.pane", json!({"pane": "format"}));
    });
}
