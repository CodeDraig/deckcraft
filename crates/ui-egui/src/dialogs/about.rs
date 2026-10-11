//! Application identity and credits window.

use super::{Dialog, buttons};
use crate::SlideApp;
use crate::theme::{self, Tokens};
use egui::{Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};

pub fn about(_app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.vertical_centered(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
        paint_logo(ui.painter(), r);
        ui.label(egui::RichText::new("DeckCraft").font(theme::bold(22.0)));
        ui.label(egui::RichText::new(version_line()).color(t.text_dim));
        ui.add_space(6.0);
        ui.label("Presentations and slide shows, rebuilt from scratch in pure Rust.");
        ui.label("An open-source, clean-room project. Not affiliated with Microsoft.");
        ui.add_space(6.0);
        ui.hyperlink_to("Join our community on Discord", deckcraft_engine::links::DISCORD);
        ui.hyperlink_to("getartcraft.com/apps/deckcraft", deckcraft_engine::links::APP_PAGE);
        ui.hyperlink_to("Source on GitHub", deckcraft_engine::links::GITHUB);
        ui.add_space(6.0);
        ui.label(egui::RichText::new("MIT OR Apache-2.0 · © 2026 ArtCraft Team and the DeckCraft contributors").size(11.0).color(t.text_faint));
    });
}

/// "Version X.Y.Z" plus, for release builds, the short commit and build date. The version comes
/// from `[workspace.package] version` (the single source of truth, `cargo xtask version`); the
/// release workflow sets `DECKCRAFT_BUILD_SHA` / `DECKCRAFT_BUILD_DATE` at build time.
pub fn version_line() -> String {
    let mut s = format!("Version {}", env!("CARGO_PKG_VERSION"));
    let sha = option_env!("DECKCRAFT_BUILD_SHA").map(|s| s.get(..9).unwrap_or(s)).filter(|s| !s.is_empty());
    let date = option_env!("DECKCRAFT_BUILD_DATE").filter(|s| !s.is_empty());
    match (sha, date) {
        (Some(sha), Some(date)) => s.push_str(&format!(" ({sha}, {date})")),
        (Some(x), None) | (None, Some(x)) => s.push_str(&format!(" ({x})")),
        (None, None) => {}
    }
    s
}

/// DeckCraft's mark, drawn in code: a slide card with a play triangle in our orange.
pub fn paint_logo(p: &egui::Painter, r: Rect) {
    let orange = Color32::from_rgb(0xF2, 0x6B, 0x1D);
    let deep = Color32::from_rgb(0xC2, 0x4E, 0x14);
    let card = r.shrink(r.width() * 0.08);
    p.rect_filled(card.translate(vec2(r.width() * 0.04, r.width() * 0.05)), CornerRadius::same((r.width() * 0.14) as u8), deep);
    p.rect_filled(card, CornerRadius::same((r.width() * 0.14) as u8), orange);
    let c = card.center();
    let s = card.width() * 0.22;
    p.add(egui::Shape::convex_polygon(
        vec![pos2(c.x - s * 0.7, c.y - s), pos2(c.x + s, c.y), pos2(c.x - s * 0.7, c.y + s)],
        Color32::WHITE,
        Stroke::NONE,
    ));
}

pub(super) fn about_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let tab = d.get("tab", "about");
    ui.horizontal(|ui| {
        for (k, l) in [("about", "About"), ("contributors", "Contributors"), ("models", "Models")] {
            if ui.selectable_label(tab == k, l).clicked() {
                d.fields.insert("tab".into(), k.into());
            }
        }
    });
    ui.separator();
    match tab.as_str() {
        "contributors" | "models" => {
            // The credits need room for the table; the About tab keeps its compact size.
            ui.set_min_size(vec2(640.0, 380.0));
            ui.set_max_height(380.0);
            if tab == "models" { crate::credits::models_ui(ui) } else { crate::credits::contributors_ui(ui) }
        }
        _ => about(app, ui),
    }
    let (ok, _) = buttons(ui, "Close");
    ok
}
