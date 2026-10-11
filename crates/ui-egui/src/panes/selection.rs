//! Selection task pane.

use crate::SlideApp;
use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};
use egui::{Align2, CornerRadius, Rect, Sense, Ui, pos2, vec2};
use serde_json::json;

pub(super) fn selection(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else { return };
    let shapes: Vec<(u32, String, bool, &'static str)> =
        st.shapes().iter().rev().map(|s| (s.id.0, s.name.clone(), s.hidden, s.kind_name())).collect();
    let selected: Vec<u32> = st.selection.shapes.iter().map(|i| i.0).collect();
    ui.horizontal(|ui| {
        if ui.button("Show All").clicked() {
            let ids: Vec<u32> = shapes.iter().map(|s| s.0).collect();
            let _ = app.run("shape.visible", json!({"ids": ids, "visible": true}));
        }
        if ui.button("Hide All").clicked() {
            let ids: Vec<u32> = shapes.iter().map(|s| s.0).collect();
            let _ = app.run("shape.visible", json!({"ids": ids, "visible": false}));
        }
    });
    ui.separator();
    let n = shapes.len();
    for (k, (id, name, hidden, kind)) in shapes.into_iter().enumerate() {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
        let sel = selected.contains(&id);
        if sel || resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::same(4), if sel { t.selected } else { t.hover });
        }
        ui.painter().text(
            pos2(r.min.x + 8.0, r.center().y),
            Align2::LEFT_CENTER,
            &name,
            theme::font(12.5),
            if hidden { t.text_faint } else { t.text },
        );
        ui.painter().text(pos2(r.max.x - 34.0, r.center().y), Align2::RIGHT_CENTER, kind, theme::font(10.5), t.text_faint);
        let eye = Rect::from_center_size(pos2(r.max.x - 14.0, r.center().y), vec2(16.0, 16.0));
        icons::paint(ui.painter(), eye, if hidden { Icon::EyeOff } else { Icon::Eye }, t.text_dim, false);
        let eye_resp = ui.interact(eye, ui.id().with(("eye", id)), Sense::click());
        if eye_resp.clicked() {
            let _ = app.run("shape.visible", json!({"id": id, "visible": hidden}));
        } else if resp.clicked() {
            let add = ui.input(|i| i.modifiers.command || i.modifiers.shift);
            let _ = app.run("edit.select", json!({"ids": [id], "toggle": add}));
        }
        if resp.double_clicked() {
            let mut d = crate::dialogs::Dialog::new("renameShape");
            d.params = json!({"id": id, "name": name});
            app.dialog = Some(d);
        }
        resp.context_menu(|ui| {
            if ui.button("Bring Forward").clicked() {
                let _ = app.run("arrange.bringForward", json!({"ids": [id]}));
                ui.close();
            }
            if ui.button("Send Backward").clicked() {
                let _ = app.run("arrange.sendBackward", json!({"ids": [id]}));
                ui.close();
            }
            if ui.button("Rename…").clicked() {
                let mut d = crate::dialogs::Dialog::new("renameShape");
                d.params = json!({"id": id, "name": name});
                app.dialog = Some(d);
                ui.close();
            }
        });
        let _ = (k, n);
    }
}
