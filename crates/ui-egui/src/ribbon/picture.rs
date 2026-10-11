//! Picture adjustment and crop controls.

use super::arrange::{arrange_group, size_group};
use super::shapes::shapes_gallery;
use super::{big, small};
use crate::SlideApp;
use crate::icons::Icon;
use crate::widgets::{big_button, group, rows};
use egui::Ui;
use serde_json::json;

pub(super) fn picture_format(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        let r = big_button(ui, Icon::RemoveBackground, "Remove\nBackground", false);
        let _ = r.on_hover_text("Remove Background (coming soon)");
        let r = big_button(ui, Icon::Corrections, "Corrections", true);
        egui::Popup::menu(&r).show(|ui| {
            for b in [-0.4, -0.2, 0.0, 0.2, 0.4] {
                if ui.button(format!("Brightness {:+}%", (b * 100.0) as i32)).clicked() {
                    let _ = app.run("picture.adjust", json!({"brightness": b}));
                }
            }
            ui.separator();
            for c in [-0.4, -0.2, 0.0, 0.2, 0.4] {
                if ui.button(format!("Contrast {:+}%", (c * 100.0) as i32)).clicked() {
                    let _ = app.run("picture.adjust", json!({"contrast": c}));
                }
            }
        });
        let r = big_button(ui, Icon::ColorAdjust, "Color", true);
        egui::Popup::menu(&r).show(|ui| {
            for s in [0.0, 0.33, 0.66, 1.0, 1.5, 2.0, 3.0] {
                if ui.button(format!("Saturation {}%", (s * 100.0) as i32)).clicked() {
                    let _ = app.run("picture.adjust", json!({"saturation": s}));
                }
            }
            if ui.button("Grayscale").clicked() {
                let _ = app.run("picture.adjust", json!({"grayscale": true}));
            }
        });
        let r = big_button(ui, Icon::Transparency, "Transparency", true);
        egui::Popup::menu(&r).show(|ui| {
            for tr in [0.0, 0.15, 0.3, 0.5, 0.65, 0.8, 0.95] {
                if ui.button(format!("{}%", (tr * 100.0) as i32)).clicked() {
                    let _ = app.run("picture.adjust", json!({"transparency": tr}));
                }
            }
        });
        rows(ui, |ui| {
            small(app, ui, Icon::ChangePicture, "Change Picture", "app.insertPictureDialog", json!({}));
            small(app, ui, Icon::ResetPicture, "Reset Picture", "picture.reset", json!({}));
        });
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::QuickStyles, "Picture\nStyles", true);
        egui::Popup::menu(&r).show(|ui| {
            if ui.button("Simple Frame, White").clicked() {
                let _ = app.run("shape.line", json!({"color": "#FFFFFF", "width": 6}));
                let _ = app.run("shape.effects", json!({"shadow": "outer"}));
            }
            if ui.button("Drop Shadow Rectangle").clicked() {
                let _ = app.run("shape.effects", json!({"shadow": "bottom"}));
            }
            if ui.button("Rounded Diagonal Corner").clicked() {
                let _ = app.run("shape.change", json!({"preset": "round2DiagRect"}));
            }
            if ui.button("Soft Edge Rectangle").clicked() {
                let _ = app.run("shape.effects", json!({"softEdges": 10}));
            }
            if ui.button("Reflected Rounded Rectangle").clicked() {
                let _ = app.run("shape.change", json!({"preset": "roundRect"}));
                let _ = app.run("shape.effects", json!({"reflection": "half"}));
            }
            if ui.button("Oval").clicked() {
                let _ = app.run("shape.change", json!({"preset": "ellipse"}));
            }
        });
        big(app, ui, Icon::AltText, "Alt\nText", "app.dialog", json!({"id": "altText"}));
    });
    arrange_group(app, ui);
    group(ui, |ui| {
        let r = big_button(ui, Icon::Crop, "Crop", true);
        egui::Popup::menu(&r).show(|ui| {
            ui.label("Crop to Shape");
            if let Some(p) = shapes_gallery(ui) {
                let _ = app.run("shape.change", json!({"preset": p}));
            }
            ui.separator();
            for (l, v) in [("Crop 10% each side", 0.1), ("Crop 20% each side", 0.2), ("Reset Crop", 0.0)] {
                if ui.button(l).clicked() {
                    let _ = app.run("picture.crop", json!({"left": v, "top": v, "right": v, "bottom": v}));
                }
            }
        });
    });
    size_group(app, ui);
}
