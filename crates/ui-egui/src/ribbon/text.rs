//! Text-formatting ribbon controls.

use super::{cref_param, enabled, scheme, toggle};
use crate::SlideApp;
use crate::icons::Icon;
use crate::widgets::{self, color_split, dropdown, group, icon_toggle, rows, small_button_ex};
use egui::{Color32, Ui};
use serde_json::{Value, json};

pub fn font_families(app: &SlideApp) -> Vec<String> {
    let mut v = vec![];
    if let Some(m) = app.session.active().and_then(|d| d.doc.masters.first()) {
        v.push(m.theme.fonts.major.latin.clone());
        v.push(m.theme.fonts.minor.latin.clone());
    }
    v.extend(deckcraft_fonts::FontDb::global().families());
    v.dedup();
    v
}

pub(super) fn font_group(app: &mut SlideApp, ui: &mut Ui, st: &Value) {
    let en = enabled(app, "format.bold");
    group(ui, |ui| {
        rows(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let font = st.get("font").and_then(Value::as_str).unwrap_or("").to_string();
                let size = st.get("size").and_then(Value::as_f64);
                let fams = font_families(app);
                let theme_fonts: Vec<String> = fams.iter().take(2).cloned().collect();
                dropdown(ui, "font", &font, 150.0, en, |ui| {
                    for (k, f) in fams.iter().enumerate() {
                        let label = if k == 0 && theme_fonts.len() == 2 {
                            format!("{f} (Headings)")
                        } else if k == 1 && theme_fonts.len() == 2 {
                            format!("{f} (Body)")
                        } else {
                            f.clone()
                        };
                        if ui.button(egui::RichText::new(label)).clicked() {
                            let _ = app.run("format.font", json!({"family": f}));
                            ui.close();
                        }
                        if k == 1 {
                            ui.separator();
                        }
                    }
                });
                let size_text = size.map(|s| widgets::fmt_num(s, "")).unwrap_or_default();
                dropdown(ui, "size", &size_text, 52.0, en, |ui| {
                    for s in deckcraft_engine::cmd::format::SIZES.iter().chain([72.0, 80.0, 88.0, 96.0].iter()) {
                        if ui.button(widgets::fmt_num(*s, "")).clicked() {
                            let _ = app.run("format.size", json!({"size": s}));
                            ui.close();
                        }
                    }
                });
                toggle(app, ui, Icon::FontGrow, "Increase Font Size", "format.grow", false);
                toggle(app, ui, Icon::FontShrink, "Decrease Font Size", "format.shrink", false);
                toggle(app, ui, Icon::ClearFormatting, "Clear All Formatting", "format.clear", false);
            });
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                let b = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(false);
                toggle(app, ui, Icon::Bold, "Bold", "format.bold", b("bold"));
                toggle(app, ui, Icon::Italic, "Italic", "format.italic", b("italic"));
                toggle(app, ui, Icon::Underline, "Underline", "format.underline", b("underline"));
                toggle(app, ui, Icon::Strikethrough, "Strikethrough", "format.strikethrough", b("strike"));
                toggle(app, ui, Icon::Superscript, "Superscript", "format.superscript", b("superscript"));
                toggle(app, ui, Icon::Subscript, "Subscript", "format.subscript", b("subscript"));
                let r = icon_toggle(ui, Icon::CharSpacing, "Character Spacing", en, false);
                egui::Popup::menu(&r).show(|ui| {
                    for (label, pt) in [("Very Tight", -3.0), ("Tight", -1.5), ("Normal", 0.0), ("Loose", 3.0), ("Very Loose", 6.0)] {
                        if ui.button(label).clicked() {
                            let _ = app.run("format.spacing", json!({"pt": pt}));
                        }
                    }
                });
                let r = icon_toggle(ui, Icon::ChangeCase, "Change Case", en, false);
                egui::Popup::menu(&r).show(|ui| {
                    for (label, mode) in [
                        ("Sentence case.", "sentence"),
                        ("lowercase", "lower"),
                        ("UPPERCASE", "upper"),
                        ("Capitalize Each Word", "title"),
                        ("tOGGLE cASE", "toggle"),
                    ] {
                        if ui.button(label).clicked() {
                            let _ = app.run("format.case", json!({"mode": mode}));
                        }
                    }
                });
                let sc = scheme(app);
                let (m, a) = color_split(ui, Icon::Highlight, "Text Highlight Color", Color32::from_rgb(255, 230, 0), en);
                if m.clicked() {
                    let _ = app.run("format.highlight", json!({"color": "#FFFF00"}));
                }
                egui::Popup::menu(&a).show(|ui| {
                    if let Some(c) = widgets::color_grid(ui, &sc, Some("No Color")) {
                        let _ = app.run("format.highlight", json!({"color": c.map(|c| serde_json::to_value(cref_param(&c)).unwrap_or_default())}));
                    }
                });
                let (m, a) = color_split(ui, Icon::FontColor, "Font Color", Color32::from_rgb(0xD8, 0x3F, 0x3F), en);
                if m.clicked() {
                    let _ = app.run("format.color", json!({"color": "#C00000"}));
                }
                egui::Popup::menu(&a).show(|ui| {
                    if let Some(Some(c)) = widgets::color_grid(ui, &sc, None) {
                        let _ = app.run("format.color", json!({"color": cref_param(&c)}));
                    }
                });
            });
        });
    });
}

pub(super) fn paragraph_group(app: &mut SlideApp, ui: &mut Ui, st: &Value) {
    let en = enabled(app, "format.bullets");
    group(ui, |ui| {
        rows(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                let b = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(false);
                let (m, a) = small_button_ex(ui, Icon::Bullets, "", en, b("bullets"), true);
                if m.on_hover_text("Bullets").clicked() {
                    let _ = app.run("format.bullets", json!({}));
                }
                if let Some(a) = a {
                    egui::Popup::menu(&a).show(|ui| {
                        if ui.button("None").clicked() {
                            let _ = app.run("format.bullets", json!({"on": false}));
                        }
                        for ch in ["•", "○", "■", "□", "◆", "➢", "✓", "–", "★"] {
                            if ui.button(egui::RichText::new(format!("{ch}  Bullet")).size(14.0)).clicked() {
                                let _ = app.run("format.bullets", json!({"on": true, "char": ch}));
                            }
                        }
                    });
                }
                let (m, a) = small_button_ex(ui, Icon::Numbering, "", en, b("numbering"), true);
                if m.on_hover_text("Numbering").clicked() {
                    let _ = app.run("format.numbering", json!({}));
                }
                if let Some(a) = a {
                    egui::Popup::menu(&a).show(|ui| {
                        if ui.button("None").clicked() {
                            let _ = app.run("format.numbering", json!({"on": false}));
                        }
                        for (label, sch) in [
                            ("1. 2. 3.", "arabicPeriod"),
                            ("1) 2) 3)", "arabicParenR"),
                            ("I. II. III.", "romanUcPeriod"),
                            ("i. ii. iii.", "romanLcPeriod"),
                            ("A. B. C.", "alphaUcPeriod"),
                            ("a) b) c)", "alphaLcParenR"),
                            ("a. b. c.", "alphaLcPeriod"),
                        ] {
                            if ui.button(label).clicked() {
                                let _ = app.run("format.numbering", json!({"on": true, "scheme": sch}));
                            }
                        }
                    });
                }
                toggle(app, ui, Icon::DecreaseIndent, "Decrease List Level", "format.outdent", false);
                toggle(app, ui, Icon::IncreaseIndent, "Increase List Level", "format.indent", false);
                let r = icon_toggle(ui, Icon::LineSpacing, "Line Spacing", en, false);
                egui::Popup::menu(&r).show(|ui| {
                    for v in [1.0, 1.5, 2.0, 2.5, 3.0] {
                        if ui.button(format!("{v:.1}")).clicked() {
                            let _ = app.run("format.lineSpacing", json!({"lines": v}));
                        }
                    }
                    if ui.button("Line Spacing Options…").clicked() {
                        app.dialog = Some(crate::dialogs::Dialog::new("paragraph"));
                    }
                });
                let r = icon_toggle(ui, Icon::Columns, "Columns", en, false);
                egui::Popup::menu(&r).show(|ui| {
                    for n in 1..=3 {
                        if ui.button(format!("{n} Column{}", if n > 1 { "s" } else { "" })).clicked() {
                            let _ = app.run("format.columns", json!({"count": n, "spacing": 18}));
                        }
                    }
                });
            });
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                let al = st.get("align").and_then(Value::as_str).unwrap_or("");
                toggle(app, ui, Icon::AlignLeft, "Align Left", "format.alignLeft", al == "l");
                toggle(app, ui, Icon::AlignCenter, "Center", "format.alignCenter", al == "ctr");
                toggle(app, ui, Icon::AlignRight, "Align Right", "format.alignRight", al == "r");
                toggle(app, ui, Icon::Justify, "Justify", "format.justify", al == "just");
                let r = icon_toggle(ui, Icon::TextDirection, "Text Direction", en, false);
                egui::Popup::menu(&r).show(|ui| {
                    for (l, d) in [
                        ("Horizontal", "horizontal"),
                        ("Rotate all text 90°", "rotate90"),
                        ("Rotate all text 270°", "rotate270"),
                        ("Stacked", "stacked"),
                    ] {
                        if ui.button(l).clicked() {
                            let _ = app.run("format.direction", json!({"dir": d}));
                        }
                    }
                });
                let r = icon_toggle(ui, Icon::AlignText, "Align Text", en, false);
                egui::Popup::menu(&r).show(|ui| {
                    for (l, a) in [("Top", "top"), ("Middle", "middle"), ("Bottom", "bottom")] {
                        if ui.button(l).clicked() {
                            let _ = app.run("format.anchor", json!({"anchor": a}));
                        }
                    }
                });
            });
        });
    });
}
