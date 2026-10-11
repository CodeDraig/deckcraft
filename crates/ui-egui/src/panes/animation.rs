//! Animation task pane.

use crate::SlideApp;
use crate::icons::{self, Icon};
use crate::ribbon::paint_anim_tile;
use crate::theme::{self, Tokens};
use deckcraft_model::{AnimClass, AnimStart};
use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

pub(super) fn animation(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else { return };
    let Some(slide) = st.current_slide() else { return };
    let anims = slide.animations.clone();
    let names: Vec<(u32, String)> = st.shapes().iter().map(|s| (s.id.0, s.name.clone())).collect();
    let selected: Vec<u32> = st.selection.shapes.iter().map(|i| i.0).collect();
    ui.horizontal(|ui| {
        if ui.button("▶ Play All").clicked() {
            let from = app.session.active().map(|d| d.selection.slide).unwrap_or(0);
            app.start_show(from, true);
            if let Some(s) = app.show.as_mut() {
                s.preview_only = true;
                s.autoplay = true;
            }
        }
    });
    ui.separator();
    if anims.is_empty() {
        ui.label(egui::RichText::new("Select an object on the slide and add an animation from the Animations tab.").color(t.text_dim));
        return;
    }
    let mut click = 0;
    for (i, a) in anims.iter().enumerate() {
        if a.start == AnimStart::OnClick {
            click += 1;
        }
        let name = names.iter().find(|(id, _)| *id == a.shape.0).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("Shape {}", a.shape));
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
        let sel = selected.contains(&a.shape.0);
        if sel || resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::same(4), if sel { t.selected } else { t.hover });
        }
        let p = ui.painter();
        p.text(
            pos2(r.min.x + 6.0, r.center().y),
            Align2::LEFT_CENTER,
            if a.start == AnimStart::OnClick { click.to_string() } else { String::new() },
            theme::font(12.0),
            t.text_dim,
        );
        let icon = match a.class {
            AnimClass::Entrance => Icon::AnimEntrance,
            AnimClass::Emphasis => Icon::AnimEmphasis,
            AnimClass::Exit => Icon::AnimExit,
            _ => Icon::AnimPath,
        };
        icons::paint(p, Rect::from_center_size(pos2(r.min.x + 30.0, r.center().y), vec2(18.0, 18.0)), icon, t.text, false);
        p.text(pos2(r.min.x + 44.0, r.center().y), Align2::LEFT_CENTER, &name, theme::font(12.5), t.text);
        // Timeline bar.
        let bar_x = r.max.x - 80.0;
        let w = (a.duration_ms as f32 / 1000.0 * 20.0).clamp(4.0, 70.0);
        let col = match a.class {
            AnimClass::Entrance => Color32::from_rgb(0x2E, 0xA3, 0x6F),
            AnimClass::Emphasis => Color32::from_rgb(0xE3, 0xB2, 0x1C),
            AnimClass::Exit => Color32::from_rgb(0xD8, 0x3F, 0x6B),
            _ => Color32::from_rgb(0x2E, 0x6F, 0xD8),
        };
        p.rect_filled(
            Rect::from_min_size(pos2(bar_x + (a.delay_ms as f32 / 1000.0 * 20.0).min(40.0), r.center().y - 4.0), vec2(w, 8.0)),
            CornerRadius::same(2),
            col,
        );
        if resp.clicked() {
            let _ = app.run("edit.select", json!({"ids": [a.shape.0]}));
        }
        resp.context_menu(|ui| {
            for (l, v) in [("Start On Click", "onClick"), ("Start With Previous", "withPrevious"), ("Start After Previous", "afterPrevious")] {
                if ui.button(l).clicked() {
                    let _ = app.run("animation.timing", json!({"index": i, "start": v}));
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Move Up").clicked() {
                let _ = app.run("animation.move", json!({"index": i, "to": i.saturating_sub(1)}));
                ui.close();
            }
            if ui.button("Move Down").clicked() {
                let _ = app.run("animation.move", json!({"index": i, "to": i + 1}));
                ui.close();
            }
            if ui.button("Remove").clicked() {
                let _ = app.run("animation.remove", json!({"index": i}));
                ui.close();
            }
        });
    }
    let _ = paint_anim_tile;
    let _ = Stroke::NONE;
}
