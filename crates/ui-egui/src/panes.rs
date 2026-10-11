//! Task panes on the right: Format Shape, Format Background, Animation, Selection, Comments and
//! Design Ideas.

mod designer;
use designer::designer;

mod comments;
use comments::comments;

mod selection;
use selection::selection;

mod animation;
use animation::animation;

mod background;
use background::background;

mod format;
use format::format_shape;

use egui::{CornerRadius, Sense, Ui, vec2};
use serde_json::Value;

use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};
use crate::{SlideApp, widgets};

pub fn show(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(pane) = app.ui.pane.clone() else { return };
    egui::Panel::right("task_pane")
        .resizable(true)
        .default_size(app.ui.pane_width)
        .size_range(240.0..=520.0)
        .frame(egui::Frame::NONE.fill(t.card).inner_margin(egui::Margin::same(10)))
        .show(ui, |ui| {
            app.ui.pane_width = ui.available_width() + 20.0;
            let title = match pane.as_str() {
                "format" => "Format Shape",
                "background" => "Format Background",
                "animation" => "Animation Pane",
                "selection" => "Selection",
                "comments" => "Comments",
                "designer" => "Design Ideas",
                _ => "Pane",
            };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(title).font(theme::bold(16.0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, CornerRadius::same(4), t.hover);
                    }
                    icons::paint(ui.painter(), r.shrink(3.0), Icon::Close, t.text_dim, false);
                    if resp.clicked() {
                        app.ui.pane = None;
                    }
                });
            });
            ui.add_space(6.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match pane.as_str() {
                "format" => format_shape(app, ui),
                "background" => background(app, ui),
                "animation" => animation(app, ui),
                "selection" => selection(app, ui),
                "comments" => comments(app, ui),
                "designer" => designer(app, ui),
                _ => {}
            });
        });
}

fn scheme(app: &SlideApp) -> deckcraft_color::ColorScheme {
    app.session.active().and_then(|d| d.doc.masters.first().map(|m| m.scheme())).unwrap_or_else(|| deckcraft_model::Theme::default().colors)
}

fn color_button(
    app: &mut SlideApp,
    ui: &mut Ui,
    label: &str,
    none_label: Option<&str>,
    on_pick: impl Fn(Option<deckcraft_model::ColorRef>) -> (String, Value),
) {
    let sc = scheme(app);
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = widgets::drop_button(ui, "Color", vec2(70.0, 22.0), true);
            egui::Popup::menu(&r).show(|ui| {
                if let Some(c) = widgets::color_grid(ui, &sc, none_label) {
                    let (id, p) = on_pick(c);
                    let _ = app.run(&id, p);
                }
            });
        });
    });
}
