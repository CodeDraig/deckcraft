//! Window chrome and quick-access actions.

use super::enabled;
use crate::SlideApp;
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::widgets::icon_toggle;
use egui::{Align2, CornerRadius, Rect, Sense, Ui, pos2, vec2};
use serde_json::json;

pub const TITLE_H: f32 = 38.0;

/// The window's title row: traffic-light space, quick access, centred title, actions.
pub fn title_bar(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("title_bar").exact_size(TITLE_H).frame(egui::Frame::NONE.fill(t.chrome)).show(ui, |ui| {
        let rect = ui.max_rect();
        // Dragging the empty title bar moves the window.
        let drag = ui.interact(rect, ui.id().with("titledrag"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            let max = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
        }
        let left = if app.integrated_titlebar { 84.0 } else { 10.0 };
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_max(pos2(rect.min.x + left, rect.min.y), pos2(rect.min.x + 420.0, rect.max.y)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        child.spacing_mut().item_spacing.x = 2.0;
        let has_doc = app.session.active().is_some();
        let dirty = app.session.active().is_some_and(|d| d.is_dirty());
        if icon_toggle(&mut child, Icon::Home, "Home", true, false).clicked() {
            let _ = app.run("file.new", json!({}));
        }
        if icon_toggle(&mut child, Icon::Save, "Save (⌘S)", has_doc, false).clicked() {
            app.save();
        }
        if icon_toggle(&mut child, Icon::Undo, "Undo (⌘Z)", enabled(app, "edit.undo"), false).clicked() {
            let _ = app.run("edit.undo", json!({}));
        }
        if icon_toggle(&mut child, Icon::Redo, "Redo (⌘Y)", enabled(app, "edit.redo"), false).clicked() {
            let _ = app.run("edit.redo", json!({}));
        }
        if icon_toggle(&mut child, Icon::PlayFromStart, "Play from Start (F5)", has_doc, false).clicked() {
            app.start_show(0, false);
        }
        let title =
            app.session.active().map(|d| format!("{}{}", d.title(), if dirty { " — Edited" } else { "" })).unwrap_or_else(|| "DeckCraft".into());
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, title, theme::font(13.0), t.text);
        // Right side.
        let mut right = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_max(pos2(rect.max.x - 320.0, rect.min.y), pos2(rect.max.x - 10.0, rect.max.y)))
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        right.spacing_mut().item_spacing.x = 6.0;
        if icon_toggle(&mut right, Icon::Search, "Search commands (⇧⌘P)", true, false).clicked() {
            app.palette = Some((String::new(), 0));
        }
        let share = right.add(
            egui::Button::new(egui::RichText::new("  Present  ").color(t.accent_text).font(theme::bold(13.0)))
                .fill(t.accent)
                .corner_radius(CornerRadius::same(13))
                .min_size(vec2(84.0, 26.0)),
        );
        if share.clicked() && has_doc {
            let from = app.session.active().map(|d| d.selection.slide).unwrap_or(0);
            app.start_show(from, false);
        }
        if icon_toggle(&mut right, Icon::CommentsBubble, "Comments", has_doc, app.ui.pane.as_deref() == Some("comments")).clicked() {
            let _ = app.run("view.pane", json!({"pane": "comments", "toggle": true}));
        }
        if icon_toggle(
            &mut right,
            if app.ui.brightness == theme::Brightness::Dark { Icon::Sparkle } else { Icon::Palette },
            "Light/Dark appearance",
            true,
            false,
        )
        .clicked()
        {
            let _ = app.run("view.dark", json!({}));
        }
    });
}
