//! New and recent presentation screen.

use super::about::paint_logo;
use crate::theme::{self, Tokens};
use crate::{SlideApp, ribbon};
use egui::{Align2, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

/// The start screen (no presentation open): new from themes, open, recent files.
pub fn start_screen(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
            paint_logo(ui.painter(), r);
            ui.label(egui::RichText::new("DeckCraft").font(theme::bold(26.0)));
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("New presentation").font(theme::bold(16.0)));
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.add_space(40.0);
            for th in deckcraft_model::theme::builtin_themes() {
                let (r, resp) = ui.allocate_exact_size(vec2(180.0, 128.0), Sense::click());
                let tile = Rect::from_min_size(r.min, vec2(180.0, 101.0));
                ribbon::paint_theme_tile(ui.painter(), tile, &th);
                if resp.hovered() {
                    ui.painter().rect_stroke(tile, CornerRadius::same(2), Stroke::new(2.0, t.accent), egui::StrokeKind::Outside);
                }
                ui.painter().text(pos2(r.min.x, r.max.y - 10.0), Align2::LEFT_CENTER, &th.name, theme::font(12.0), t.text);
                if resp.clicked() {
                    let _ = app.run("file.new", json!({"theme": th.name}));
                }
            }
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            if ui.button("Open…").clicked() {
                let _ = app.run("app.openDialog", json!({}));
            }
            if ui.button("Open the sample deck").clicked()
                && let Err(e) = deckcraft_engine::sample::open_sample(&mut app.session)
            {
                app.set_status(e.to_string());
            }
        });
        if !app.ui.recent.is_empty() {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(40.0);
                ui.label(egui::RichText::new("Recent").font(theme::bold(16.0)));
            });
            for p in app.ui.recent.clone() {
                ui.horizontal(|ui| {
                    ui.add_space(40.0);
                    if ui.link(&p).clicked()
                        && let Err(e) = app.open_path(&p)
                    {
                        app.set_status(e);
                    }
                });
            }
        }
        ui.add_space(30.0);
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            ui.hyperlink_to("Join the ArtCraft community on Discord", deckcraft_engine::links::DISCORD);
        });
    });
}
