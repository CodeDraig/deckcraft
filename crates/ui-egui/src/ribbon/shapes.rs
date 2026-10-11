//! Shape gallery previews and drawing-tool selection.

use crate::SlideApp;
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::widgets::{big_button, big_button_ex, group};
use egui::{Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};

/// Shapes gallery popup body: every preset by category. Returns the chosen preset.
pub fn shapes_gallery(ui: &mut Ui) -> Option<&'static str> {
    let t = Tokens::get(ui.ctx());
    let mut out = None;
    ui.set_max_width(330.0);
    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
        for cat in deckcraft_geom::preset::Category::ALL {
            ui.label(egui::RichText::new(cat.label()).font(theme::bold(12.0)));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(1.0, 1.0);
                for p in deckcraft_geom::preset::CATALOG.iter().filter(|p| p.category == cat) {
                    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
                    }
                    paint_preset(ui.painter(), r.shrink(4.0), p.name, Color32::TRANSPARENT, t.text);
                    if resp.on_hover_text(p.label).clicked() {
                        out = Some(p.name);
                    }
                }
                if cat == deckcraft_geom::preset::Category::Lines {
                    for (name, label) in deckcraft_engine::tools::FREEFORM_TOOLS {
                        let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
                        }
                        paint_freeform_icon(ui.painter(), r.shrink(5.0), name, t.text);
                        if resp.on_hover_text(label).clicked() {
                            out = Some(name);
                        }
                    }
                }
            });
            ui.add_space(4.0);
        }
    });
    if out.is_some() {
        ui.close();
    }
    out
}

/// Draw a preset's outline (gallery icons).
pub fn paint_preset(p: &egui::Painter, r: Rect, name: &str, fill: Color32, stroke: Color32) {
    let (w, h) = (r.width() as f64, r.height() as f64);
    let Some(g) = deckcraft_geom::preset::build(name, w, h, &[]) else { return };
    for sp in &g.paths {
        let mut pts: Vec<egui::Pos2> = vec![];
        let mut polys: Vec<(Vec<egui::Pos2>, bool)> = vec![];
        deckcraft_geom::preset_flatten(&sp.path, &mut |el| match el {
            deckcraft_geom::PathEl::MoveTo(point) => {
                if pts.len() > 1 {
                    polys.push((std::mem::take(&mut pts), false));
                }
                pts.clear();
                pts.push(pos2(r.min.x + point.x as f32, r.min.y + point.y as f32));
            }
            deckcraft_geom::PathEl::LineTo(point) => pts.push(pos2(r.min.x + point.x as f32, r.min.y + point.y as f32)),
            deckcraft_geom::PathEl::ClosePath if pts.len() > 1 => {
                polys.push((std::mem::take(&mut pts), true));
            }
            _ => {}
        });
        if pts.len() > 1 {
            polys.push((pts, false));
        }
        for (poly, closed) in polys {
            if closed && fill != Color32::TRANSPARENT && sp.fill != deckcraft_geom::preset::FillMode::None {
                p.add(egui::Shape::Path(egui::epaint::PathShape { points: poly.clone(), closed: true, fill, stroke: Stroke::NONE.into() }));
            }
            if sp.stroke {
                p.add(egui::Shape::Path(egui::epaint::PathShape {
                    points: poly,
                    closed,
                    fill: Color32::TRANSPARENT,
                    stroke: Stroke::new(1.0, stroke).into(),
                }));
            }
        }
    }
}

pub(super) fn draw(app: &mut SlideApp, ui: &mut Ui) {
    use deckcraft_engine::ToolKind;
    let cur = app.session.tool.kind.clone();
    group(ui, |ui| {
        let sel = matches!(cur, ToolKind::Select);
        if big_button_ex(ui, Icon::LassoSelect, "Select", true, false, sel).0.clicked() {
            app.session.set_tool(ToolKind::Select);
        }
        let er = matches!(&cur, ToolKind::Ink { mode, .. } if mode == "eraser");
        if big_button_ex(ui, Icon::Eraser, "Eraser", true, false, er).0.clicked() {
            app.session.set_tool(ToolKind::Ink { mode: "eraser".into(), color: deckcraft_color::Rgba::BLACK, width: 2.0 });
        }
    });
    group(ui, |ui| {
        let pens: [(Icon, &str, &str, deckcraft_color::Rgba, f64); 5] = [
            (Icon::Pen, "Black Pen", "pen", deckcraft_color::Rgba::rgb(20, 20, 20), 1.5),
            (Icon::Pen, "Red Pen", "pen", deckcraft_color::Rgba::rgb(0xD8, 0x3F, 0x3F), 1.5),
            (Icon::Pen, "Blue Pen", "pen", deckcraft_color::Rgba::rgb(0x2E, 0x6F, 0xD8), 1.5),
            (Icon::Pencil, "Pencil", "pen", deckcraft_color::Rgba::rgb(90, 90, 90), 1.0),
            (Icon::HighlighterPen, "Highlighter", "highlighter", deckcraft_color::Rgba::rgb(255, 230, 0), 10.0),
        ];
        for (icon, label, mode, color, width) in pens {
            let on = matches!(&cur, ToolKind::Ink { mode: m, color: c, .. } if m == mode && *c == color);
            if big_button_ex(ui, icon, label, true, false, on).0.clicked() {
                app.session.set_tool(ToolKind::Ink { mode: mode.into(), color, width });
            }
        }
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::InkToShape, "Ink to\nShape", false);
        let _ = r.on_hover_text("Ink to Shape (coming soon)");
        let r = big_button(ui, Icon::InkToText, "Ink to\nText", false);
        let _ = r.on_hover_text("Ink to Text (coming soon)");
        let r = big_button(ui, Icon::InkToMath, "Ink to\nMath", false);
        let _ = r.on_hover_text("Ink to Math (coming soon)");
    });
}

/// Gallery glyphs for the freeform tools: a wave (curve), a polygon (freeform), a squiggle (scribble).
pub(super) fn paint_freeform_icon(p: &egui::Painter, r: Rect, name: &str, ink: Color32) {
    let at = |x: f32, y: f32| egui::pos2(r.min.x + x * r.width(), r.min.y + y * r.height());
    let pts: Vec<egui::Pos2> = match name {
        "curve" => (0..=24).map(|i| i as f32 / 24.0).map(|u| at(u, 0.5 - 0.38 * (u * std::f32::consts::TAU).sin())).collect(),
        "freeform" => {
            [(0.0, 0.85), (0.15, 0.2), (0.55, 0.45), (0.8, 0.05), (1.0, 0.75), (0.45, 1.0), (0.0, 0.85)].iter().map(|(x, y)| at(*x, *y)).collect()
        }
        _ => (0..=40).map(|i| i as f32 / 40.0).map(|u| at(u, 0.5 + 0.35 * (u * 19.0).sin() * (1.0 - u * 0.4))).collect(),
    };
    p.add(egui::Shape::line(pts, egui::Stroke::new(1.3, ink)));
}
