//! Background task pane.

use super::scheme;
use crate::ribbon::cref_param;
use crate::theme::{self};
use crate::{SlideApp, widgets};
use deckcraft_model::Fill;
use egui::{Ui, vec2};
use serde_json::{Value, json};

pub(super) fn background(app: &mut SlideApp, ui: &mut Ui) {
    let sc = scheme(app);
    let bg = app.session.active().and_then(|d| d.current_slide().and_then(|s| s.background.clone()));
    let fill = match &bg {
        Some(deckcraft_model::Background::Fill { fill }) => Some(fill.clone()),
        _ => None,
    };
    let kind = match &fill {
        Some(Fill::Solid { .. }) => "solid",
        Some(Fill::Gradient(_)) => "gradient",
        Some(Fill::Picture(_)) => "picture",
        Some(Fill::Pattern(_)) => "pattern",
        _ => "auto",
    };
    // What "Apply to All" re-applies: the current slide's background as command params.
    let mut current: Option<Value> = None;
    ui.label(egui::RichText::new("Fill").font(theme::bold(13.0)));
    for (l, k) in [
        ("Solid fill", "solid"),
        ("Gradient fill", "gradient"),
        ("Picture or texture fill", "picture"),
        ("Pattern fill", "pattern"),
        ("Follow the layout", "auto"),
    ] {
        if ui.radio(kind == k, l).clicked() && kind != k {
            match k {
                "solid" => {
                    let _ = app.run("design.background", json!({"color": "bg1"}));
                }
                "gradient" => {
                    let mut g = crate::fillui::default_gradient(deckcraft_model::ColorRef::scheme(deckcraft_color::SchemeSlot::Bg2), 90.0);
                    g.rotate_with_shape = false;
                    let _ = app.run("design.background", json!({"gradient": crate::fillui::gradient_json(&g)}));
                }
                "picture" => {
                    if let Some(pick) = app.services.pick_open.as_mut()
                        && let Some(path) = pick("picture")
                        && let Some(Ok(bytes)) = app.services.read.as_ref().map(|r| r(&path))
                    {
                        let _ = app.run("design.background", json!({"picture": deckcraft_engine::cmd::base64_encode(&bytes)}));
                    }
                }
                "pattern" => {
                    let _ = app.run("design.background", json!({"pattern": {"preset": "pct20", "fg": "accent1", "bg": "bg1"}}));
                }
                _ => {
                    let _ = app.run("design.background", json!({"reset": true}));
                }
            }
        }
    }
    ui.add_space(4.0);
    match &fill {
        Some(Fill::Solid { color }) => {
            current = Some(json!({"color": cref_param(color)}));
            ui.horizontal(|ui| {
                ui.label("Color");
                let r = widgets::drop_button(ui, "Color", vec2(70.0, 20.0), true);
                egui::Popup::menu(&r).show(|ui| {
                    if let Some(Some(c)) = widgets::color_grid(ui, &sc, None) {
                        let _ = app.run("design.background", json!({"color": cref_param(&c)}));
                    }
                });
            });
        }
        Some(Fill::Gradient(g)) => {
            current = Some(json!({"gradient": crate::fillui::gradient_json(g)}));
            if let Some(ng) = crate::fillui::gradient_editor(ui, "bg-grad", g, &sc) {
                let _ = app.run("design.background", json!({"gradient": crate::fillui::gradient_json(&ng)}));
            }
        }
        Some(Fill::Pattern(pt)) => {
            current = Some(json!({"pattern": {"preset": pt.preset, "fg": cref_param(&pt.fg), "bg": cref_param(&pt.bg)}}));
            ui.horizontal_wrapped(|ui| {
                for preset in ["pct5", "pct20", "pct50", "ltHorz", "ltVert", "smGrid", "dkDnDiag", "wave", "dotGrid", "weave"] {
                    if ui.selectable_label(pt.preset == preset, preset).clicked() {
                        let _ =
                            app.run("design.background", json!({"pattern": {"preset": preset, "fg": cref_param(&pt.fg), "bg": cref_param(&pt.bg)}}));
                    }
                }
            });
            for (label, fg) in [("Foreground", true), ("Background", false)] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    let r = widgets::drop_button(ui, "Color", vec2(70.0, 20.0), true);
                    egui::Popup::menu(&r).show(|ui| {
                        if let Some(Some(c)) = widgets::color_grid(ui, &sc, None) {
                            let (f, b) = if fg { (c, pt.bg.clone()) } else { (pt.fg.clone(), c) };
                            let _ =
                                app.run("design.background", json!({"pattern": {"preset": pt.preset, "fg": cref_param(&f), "bg": cref_param(&b)}}));
                        }
                    });
                });
            }
        }
        Some(Fill::Picture(_)) => {
            if ui.button("Insert picture from file…").clicked()
                && let Some(pick) = app.services.pick_open.as_mut()
                && let Some(path) = pick("picture")
                && let Some(Ok(bytes)) = app.services.read.as_ref().map(|r| r(&path))
            {
                let _ = app.run("design.background", json!({"picture": deckcraft_engine::cmd::base64_encode(&bytes)}));
            }
        }
        _ => {}
    }
    ui.add_space(4.0);
    let hide = app.session.active().and_then(|d| d.current_slide().map(|s| !s.show_master_shapes)).unwrap_or(false);
    let mut h = hide;
    if ui.checkbox(&mut h, "Hide background graphics").changed() {
        let _ = app.run("design.hideBackgroundGraphics", json!({"hide": h}));
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(current.is_some(), egui::Button::new("Apply to All")).clicked()
            && let Some(Value::Object(mut o)) = current.clone()
        {
            o.insert("all".into(), json!(true));
            let _ = app.run("design.background", Value::Object(o));
        }
        if ui.button("Reset Background").clicked() {
            let _ = app.run("design.background", json!({"reset": true}));
        }
    });
}
