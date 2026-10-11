//! Object arrangement, alignment and bounds controls.

use super::{big, enabled};
use crate::SlideApp;
use crate::icons::Icon;
use crate::theme::{self};
use crate::widgets::{self, big_button, big_button_ex, group, rows, small_button, small_button_ex};
use egui::Ui;
use serde_json::{Value, json};

pub fn arrange_menu(app: &mut SlideApp, ui: &mut Ui) {
    ui.label(egui::RichText::new("Order Objects").font(theme::bold(12.0)));
    for (l, id, ic) in [
        ("Bring to Front", "arrange.bringToFront", Icon::BringToFront),
        ("Send to Back", "arrange.sendToBack", Icon::SendToBack),
        ("Bring Forward", "arrange.bringForward", Icon::BringForward),
        ("Send Backward", "arrange.sendBackward", Icon::SendBackward),
    ] {
        menu_item(app, ui, ic, l, id, json!({}));
    }
    ui.separator();
    ui.label(egui::RichText::new("Group Objects").font(theme::bold(12.0)));
    menu_item(app, ui, Icon::Group, "Group", "arrange.group", json!({}));
    menu_item(app, ui, Icon::Ungroup, "Ungroup", "arrange.ungroup", json!({}));
    menu_item(app, ui, Icon::Group, "Regroup", "arrange.regroup", json!({}));
    ui.separator();
    ui.label(egui::RichText::new("Position Objects").font(theme::bold(12.0)));
    ui.menu_button("Align", |ui| align_menu(app, ui));
    ui.menu_button("Rotate", |ui| {
        menu_item(app, ui, Icon::RotateRight, "Rotate Right 90°", "arrange.rotateRight", json!({}));
        menu_item(app, ui, Icon::RotateLeft, "Rotate Left 90°", "arrange.rotateLeft", json!({}));
        menu_item(app, ui, Icon::FlipV, "Flip Vertical", "arrange.flipVertical", json!({}));
        menu_item(app, ui, Icon::FlipH, "Flip Horizontal", "arrange.flipHorizontal", json!({}));
    });
    if ui.button("Selection Pane…").clicked() {
        let _ = app.run("view.pane", json!({"pane": "selection"}));
        ui.close();
    }
}

pub fn align_menu(app: &mut SlideApp, ui: &mut Ui) {
    for (l, e, ic) in [
        ("Align Left", "left", Icon::AlignObjLeft),
        ("Align Center", "center", Icon::AlignObjCenter),
        ("Align Right", "right", Icon::AlignObjRight),
        ("Align Top", "top", Icon::AlignObjTop),
        ("Align Middle", "middle", Icon::AlignObjMiddle),
        ("Align Bottom", "bottom", Icon::AlignObjBottom),
    ] {
        menu_item(app, ui, ic, l, "arrange.align", json!({"edge": e}));
    }
    ui.separator();
    menu_item(app, ui, Icon::DistributeH, "Distribute Horizontally", "arrange.distribute", json!({"dir": "horizontal"}));
    menu_item(app, ui, Icon::DistributeV, "Distribute Vertically", "arrange.distribute", json!({"dir": "vertical"}));
}

pub fn menu_item(app: &mut SlideApp, ui: &mut Ui, icon: Icon, label: &str, id: &str, p: Value) {
    let en = enabled(app, id);
    let r = small_button(ui, icon, label, en, false);
    if r.clicked() {
        let _ = app.run(id, p);
        ui.close();
    }
}

pub(super) fn arrange_group(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        let sel = enabled(app, "arrange.bringForward");
        let (m, a) = big_button_ex(ui, Icon::BringForward, "Bring\nForward", sel, true, false);
        if m.clicked() {
            let _ = app.run("arrange.bringForward", json!({}));
        }
        if let Some(a) = a {
            egui::Popup::menu(&a).show(|ui| {
                menu_item(app, ui, Icon::BringForward, "Bring Forward", "arrange.bringForward", json!({}));
                menu_item(app, ui, Icon::BringToFront, "Bring to Front", "arrange.bringToFront", json!({}));
            });
        }
        let (m, a) = big_button_ex(ui, Icon::SendBackward, "Send\nBackward", sel, true, false);
        if m.clicked() {
            let _ = app.run("arrange.sendBackward", json!({}));
        }
        if let Some(a) = a {
            egui::Popup::menu(&a).show(|ui| {
                menu_item(app, ui, Icon::SendBackward, "Send Backward", "arrange.sendBackward", json!({}));
                menu_item(app, ui, Icon::SendToBack, "Send to Back", "arrange.sendToBack", json!({}));
            });
        }
        big(app, ui, Icon::SelectionPane, "Selection\nPane", "view.pane", json!({"pane": "selection"}));
        let r = big_button(ui, Icon::AlignObjects, "Align", sel);
        egui::Popup::menu(&r).show(|ui| align_menu(app, ui));
        rows(ui, |ui| {
            let (_, a) = small_button_ex(ui, Icon::Group, "Group", sel, false, true);
            if let Some(a) = a {
                egui::Popup::menu(&a).show(|ui| {
                    menu_item(app, ui, Icon::Group, "Group", "arrange.group", json!({}));
                    menu_item(app, ui, Icon::Ungroup, "Ungroup", "arrange.ungroup", json!({}));
                    menu_item(app, ui, Icon::Group, "Regroup", "arrange.regroup", json!({}));
                });
            }
            let (_, a) = small_button_ex(ui, Icon::Rotate, "Rotate", sel, false, true);
            if let Some(a) = a {
                egui::Popup::menu(&a).show(|ui| {
                    menu_item(app, ui, Icon::RotateRight, "Rotate Right 90°", "arrange.rotateRight", json!({}));
                    menu_item(app, ui, Icon::RotateLeft, "Rotate Left 90°", "arrange.rotateLeft", json!({}));
                    menu_item(app, ui, Icon::FlipV, "Flip Vertical", "arrange.flipVertical", json!({}));
                    menu_item(app, ui, Icon::FlipH, "Flip Horizontal", "arrange.flipHorizontal", json!({}));
                });
            }
        });
    });
}

pub(super) fn size_group(app: &mut SlideApp, ui: &mut Ui) {
    let bx = app
        .session
        .active()
        .and_then(|d| d.selection.shapes.first().and_then(|id| d.shape(*id).map(|s| deckcraft_engine::cmd::xfrm_of(&d.doc, &d.selection, s))));
    group(ui, |ui| {
        rows(ui, |ui| {
            let Some(x) = bx else {
                ui.label("Height:");
                ui.label("Width:");
                return;
            };
            ui.horizontal(|ui| {
                ui.label("Height:");
                if let Some(v) = widgets::spinner(ui, "sh", x.h / 72.0, 0.1, "\"", 54.0) {
                    let _ = app.run("shape.resize", json!({"h": v.max(0.0) * 72.0}));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Width: ");
                if let Some(v) = widgets::spinner(ui, "sw", x.w / 72.0, 0.1, "\"", 54.0) {
                    let _ = app.run("shape.resize", json!({"w": v.max(0.0) * 72.0}));
                }
            });
        });
    });
}
