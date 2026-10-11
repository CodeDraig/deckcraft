//! Comments task pane.

use crate::SlideApp;
use crate::theme::{self, Tokens};
use egui::{Align2, Color32, CornerRadius, Sense, Ui, vec2};
use serde_json::json;

pub(super) fn comments(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    if ui.button("+ New Comment").clicked() {
        app.dialog = Some(crate::dialogs::Dialog::new("comment"));
    }
    ui.separator();
    let Some(st) = app.session.active() else { return };
    let list = st.current_slide().map(|s| s.comments.clone()).unwrap_or_default();
    if list.is_empty() {
        ui.label(egui::RichText::new("No comments on this slide.").color(t.text_dim));
    }
    for (i, c) in list.iter().enumerate() {
        egui::Frame::NONE.fill(t.chrome).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 12.0, t.accent);
                ui.painter().text(r.center(), Align2::CENTER_CENTER, &c.initials, theme::bold(11.0), Color32::WHITE);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&c.author).font(theme::bold(12.5)));
                    ui.label(egui::RichText::new(c.date.replace('T', " ").trim_end_matches('Z')).size(10.5).color(t.text_faint));
                });
            });
            ui.label(&c.text);
            for r in &c.replies {
                ui.label(egui::RichText::new(format!("↳ {}: {}", r.author, r.text)).color(t.text_dim));
            }
            ui.horizontal(|ui| {
                let id = ui.id().with(("reply", i));
                let mut txt: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
                let r = ui.add(egui::TextEdit::singleline(&mut txt).hint_text("Reply…").desired_width(ui.available_width() - 70.0));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !txt.trim().is_empty() {
                    let _ = app.run("comment.reply", json!({"index": i, "text": txt}));
                    txt.clear();
                }
                ui.data_mut(|d| d.insert_temp(id, txt));
                if ui.small_button(if c.resolved { "Reopen" } else { "Resolve" }).clicked() {
                    let _ = app.run("comment.resolve", json!({"index": i, "resolved": !c.resolved}));
                }
                if ui.small_button("🗑").clicked() {
                    let _ = app.run("comment.delete", json!({"index": i}));
                }
            });
        });
        ui.add_space(6.0);
    }
}
