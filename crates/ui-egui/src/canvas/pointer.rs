//! Canvas pointer translation, cursor and contextual commands.

use super::Xf;
use crate::SlideApp;
use deckcraft_engine::tools::Hit;
use deckcraft_engine::{Mods, PointerEvent, PointerKind, ToolKind};
use egui::Ui;
use serde_json::json;

pub(super) fn mods(ui: &Ui) -> Mods {
    ui.input(|i| Mods { shift: i.modifiers.shift, alt: i.modifiers.alt, cmd: i.modifiers.command || i.modifiers.ctrl })
}

pub(super) fn pointer(app: &mut SlideApp, ui: &Ui, resp: &egui::Response, xf: Xf) {
    let tol = (4.0 / xf.scale) as f64;
    let m = mods(ui);
    let Some(pos) = resp.interact_pointer_pos().or(resp.hover_pos()) else { return };
    let p = xf.to_slide(pos);
    let ev = |kind| PointerEvent { kind, x: p.x, y: p.y, mods: m, tol };
    let primary = ui.input(|i| i.pointer.primary_pressed());
    if resp.hovered() && !app.session.tool.dragging() {
        let _ = app.session.pointer(ev(PointerKind::Move));
    }
    let r = if resp.triple_clicked() {
        app.session.pointer(ev(PointerKind::TripleClick))
    } else if resp.double_clicked() {
        app.session.pointer(ev(PointerKind::DoubleClick))
    } else if primary && resp.hovered() && !resp.secondary_clicked() && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
        app.session.pointer(ev(PointerKind::Down))
    } else if resp.dragged_by(egui::PointerButton::Primary) || (app.session.tool.dragging() && ui.input(|i| i.pointer.primary_down())) {
        app.session.pointer(ev(PointerKind::Drag))
    } else if app.session.tool.dragging() && ui.input(|i| i.pointer.primary_released() || !i.pointer.primary_down()) {
        app.session.pointer(ev(PointerKind::Up))
    } else {
        Ok(serde_json::Value::Null)
    };
    if let Err(e) = r {
        app.set_status(e.to_string());
    }
    // Cursor.
    let cursor = match (&app.session.tool.kind, &app.session.tool.hover) {
        (ToolKind::Shape { .. }, _) => egui::CursorIcon::Crosshair,
        (ToolKind::TextBox, _) => egui::CursorIcon::Text,
        (ToolKind::Ink { .. }, _) => egui::CursorIcon::Crosshair,
        (_, Some(Hit::Resize { handle, .. })) => match handle {
            0 | 4 => egui::CursorIcon::ResizeNwSe,
            2 | 6 => egui::CursorIcon::ResizeNeSw,
            1 | 5 => egui::CursorIcon::ResizeVertical,
            _ => egui::CursorIcon::ResizeHorizontal,
        },
        (_, Some(Hit::Rotate { .. })) => egui::CursorIcon::Alias,
        (_, Some(Hit::Adjust { .. } | Hit::LineEnd { .. })) => egui::CursorIcon::PointingHand,
        (_, Some(Hit::Shape { text: true, .. })) => {
            if app.session.active().is_some_and(|d| d.selection.text.is_some()) {
                egui::CursorIcon::Text
            } else {
                egui::CursorIcon::Move
            }
        }
        (_, Some(Hit::Shape { .. })) => egui::CursorIcon::Move,
        _ => egui::CursorIcon::Default,
    };
    if resp.hovered() {
        ui.ctx().set_cursor_icon(cursor);
    }
    if app.session.tool.dragging() {
        ui.ctx().request_repaint();
    }
}

pub(super) fn context_menu(app: &mut SlideApp, resp: &egui::Response) {
    resp.context_menu(|ui| {
        let has_sel = app.session.active().is_some_and(|d| !d.selection.shapes.is_empty());
        for (l, id) in [("Cut", "edit.cut"), ("Copy", "edit.copy"), ("Paste", "edit.paste")] {
            if ui.add_enabled(crate::ribbon::enabled(app, id), egui::Button::new(l)).clicked() {
                let _ = app.run(id, json!({}));
                ui.close();
            }
        }
        ui.separator();
        if has_sel {
            if ui.button("Edit Text").clicked() {
                let _ = app.run("text.edit", json!({}));
                ui.close();
            }
            ui.menu_button("Bring to Front", |ui| {
                crate::ribbon::menu_item(app, ui, crate::icons::Icon::BringToFront, "Bring to Front", "arrange.bringToFront", json!({}));
                crate::ribbon::menu_item(app, ui, crate::icons::Icon::BringForward, "Bring Forward", "arrange.bringForward", json!({}));
            });
            ui.menu_button("Send to Back", |ui| {
                crate::ribbon::menu_item(app, ui, crate::icons::Icon::SendToBack, "Send to Back", "arrange.sendToBack", json!({}));
                crate::ribbon::menu_item(app, ui, crate::icons::Icon::SendBackward, "Send Backward", "arrange.sendBackward", json!({}));
            });
            ui.menu_button("Group", |ui| {
                crate::ribbon::menu_item(app, ui, crate::icons::Icon::Group, "Group", "arrange.group", json!({}));
                crate::ribbon::menu_item(app, ui, crate::icons::Icon::Ungroup, "Ungroup", "arrange.ungroup", json!({}));
            });
            if ui.button("Link…").clicked() {
                app.dialog = Some(crate::dialogs::Dialog::new("hyperlink"));
                ui.close();
            }
            if ui.button("Edit Alt Text…").clicked() {
                app.dialog = Some(crate::dialogs::Dialog::new("altText"));
                ui.close();
            }
            if ui.button("Set as Default Shape").clicked() {
                let _ = app.run("shape.setDefault", json!({}));
                ui.close();
            }
            if ui.button("Format Shape…").clicked() {
                let _ = app.run("view.pane", json!({"pane": "format"}));
                ui.close();
            }
        } else {
            ui.menu_button("Layout", |ui| {
                let layouts: Vec<(String, u32)> = app
                    .session
                    .active()
                    .and_then(|d| d.doc.masters.first().map(|m| m.layouts.iter().map(|l| (l.name.clone(), l.id.0)).collect()))
                    .unwrap_or_default();
                for (n, id) in layouts {
                    if ui.button(n).clicked() {
                        let _ = app.run("slide.layout", json!({"layout": id}));
                        ui.close();
                    }
                }
            });
            if ui.button("Reset Slide").clicked() {
                let _ = app.run("slide.reset", json!({}));
                ui.close();
            }
            if ui.button("Format Background…").clicked() {
                let _ = app.run("view.pane", json!({"pane": "background"}));
                ui.close();
            }
            if ui.button("New Comment").clicked() {
                app.dialog = Some(crate::dialogs::Dialog::new("comment"));
                ui.close();
            }
        }
    });
}
