//! Format shape task pane.

use super::{color_button, scheme};
use crate::icons::Icon;
use crate::ribbon::cref_param;
use crate::theme::{self, Tokens};
use crate::{SlideApp, widgets};
use deckcraft_model::{Fill, ShapeKind};
use egui::{Ui, vec2};
use serde_json::{Value, json};

pub(super) fn format_shape(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else { return };
    let Some(sh) = st.selected_shapes().first().map(|s| (*s).clone()) else {
        ui.label(egui::RichText::new("Select a shape to format it.").color(t.text_dim));
        return;
    };
    let x = deckcraft_engine::cmd::xfrm_of(&st.doc, &st.selection, &sh);
    // Tabs as icons.
    ui.horizontal(|ui| {
        for (tab, icon, tip) in [
            ("fill", Icon::ShapeFill, "Fill & Line"),
            ("effects", Icon::ShapeEffects, "Effects"),
            ("size", Icon::SlideSize, "Size & Properties"),
            ("text", Icon::TextBox, "Text Options"),
        ] {
            let on = app.ui.format_tab == tab;
            if widgets::icon_toggle(ui, icon, tip, true, on).clicked() {
                app.ui.format_tab = tab.into();
            }
        }
        if matches!(sh.kind, ShapeKind::Picture { .. })
            && widgets::icon_toggle(ui, Icon::Picture, "Picture", true, app.ui.format_tab == "picture").clicked()
        {
            app.ui.format_tab = "picture".into();
        }
    });
    ui.separator();
    match app.ui.format_tab.as_str() {
        "effects" => effects(app, ui, &sh),
        "size" => size(app, ui, &sh, x),
        "text" => text(app, ui, &sh),
        "picture" => picture(app, ui, &sh),
        _ => fill_line(app, ui, &sh),
    }
}

fn effects(app: &mut SlideApp, ui: &mut Ui, sh: &deckcraft_model::Shape) {
    ui.label(egui::RichText::new("Shadow").font(theme::bold(13.0)));
    ui.horizontal_wrapped(|ui| {
        for (l, v) in
            [("None", "none"), ("Outer", "outer"), ("Bottom", "bottom"), ("Center", "center"), ("Inner", "inner"), ("Perspective", "perspective")]
        {
            if ui.button(l).clicked() {
                let _ = app.run("shape.effects", json!({"shadow": v}));
            }
        }
    });
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Reflection").font(theme::bold(13.0)));
    ui.horizontal_wrapped(|ui| {
        for (l, v) in [("None", "none"), ("Tight", "tight"), ("Half", "half"), ("Full", "full")] {
            if ui.button(l).clicked() {
                let _ = app.run("shape.effects", json!({"reflection": v}));
            }
        }
    });
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Glow").font(theme::bold(13.0)));
    let mut g = sh.effects.as_ref().and_then(|e| e.glow.as_ref().map(|g| g.radius)).unwrap_or(0.0);
    if ui.add(egui::Slider::new(&mut g, 0.0..=40.0).text("size (pt)")).drag_stopped() {
        let _ = app.run("shape.effects", json!({"glow": if g > 0.0 { json!(g) } else { Value::Null }}));
    }
    ui.label(egui::RichText::new("Soft Edges").font(theme::bold(13.0)));
    let mut s = sh.effects.as_ref().and_then(|e| e.soft_edge).unwrap_or(0.0);
    if ui.add(egui::Slider::new(&mut s, 0.0..=50.0).text("size (pt)")).drag_stopped() {
        let _ = app.run("shape.effects", json!({"softEdges": s}));
    }
    if ui.button("Reset Effects").clicked() {
        let _ = app.run("shape.effects", json!({"reset": true}));
    }
}

fn size(app: &mut SlideApp, ui: &mut Ui, sh: &deckcraft_model::Shape, x: deckcraft_geom::Xfrm) {
    let mut fields = |ui: &mut Ui, label: &str, id: &str, v: f64, key: &str, cmd: &str, unit: f64| {
        ui.horizontal(|ui| {
            ui.add_sized(vec2(90.0, 20.0), egui::Label::new(label));
            if let Some(nv) = widgets::spinner(ui, id, v / unit, 0.1, if unit == 72.0 { "\"" } else { "°" }, 70.0) {
                let _ = app.run(cmd, json!({key: nv * unit}));
            }
        });
    };
    ui.label(egui::RichText::new("Size").font(theme::bold(13.0)));
    fields(ui, "Height", "fh", x.h, "h", "shape.resize", 72.0);
    fields(ui, "Width", "fw", x.w, "w", "shape.resize", 72.0);
    fields(ui, "Rotation", "fr", x.rot, "deg", "shape.rotate", 1.0);
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Position").font(theme::bold(13.0)));
    fields(ui, "Horizontal", "fx", x.x, "x", "shape.move", 72.0);
    fields(ui, "Vertical", "fy", x.y, "y", "shape.move", 72.0);
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Alt Text").font(theme::bold(13.0)));
    let id = ui.id().with(("alt", sh.id.0));
    let mut alt: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_else(|| sh.descr.clone()));
    let r = ui.add(egui::TextEdit::multiline(&mut alt).desired_rows(3).desired_width(f32::INFINITY));
    if r.lost_focus() && alt != sh.descr {
        let _ = app.run("shape.altText", json!({"text": alt}));
    }
    ui.data_mut(|d| d.insert_temp(id, alt));
}

fn text(app: &mut SlideApp, ui: &mut Ui, sh: &deckcraft_model::Shape) {
    ui.label(egui::RichText::new("Text Box").font(theme::bold(13.0)));
    ui.horizontal(|ui| {
        ui.label("Vertical alignment");
        for (l, a) in [("Top", "top"), ("Middle", "middle"), ("Bottom", "bottom")] {
            if ui.small_button(l).clicked() {
                let _ = app.run("format.anchor", json!({"anchor": a}));
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        for (l, m) in [("Do not Autofit", "none"), ("Shrink text on overflow", "shrink"), ("Resize shape to fit text", "resize")] {
            if ui.small_button(l).clicked() {
                let _ = app.run("format.autofit", json!({"mode": m}));
            }
        }
    });
    let wrap = sh.text.as_ref().and_then(|t| t.body.wrap).unwrap_or(true);
    let mut w = wrap;
    if ui.checkbox(&mut w, "Wrap text in shape").changed() {
        let _ = app.run("format.wrap", json!({"on": w}));
    }
    ui.label("Margins (pt)");
    let b = sh.text.as_ref().map(|t| t.body.clone()).unwrap_or_default();
    for (l, k, v) in [
        ("Left", "left", b.inset_l.unwrap_or(7.2)),
        ("Right", "right", b.inset_r.unwrap_or(7.2)),
        ("Top", "top", b.inset_t.unwrap_or(3.6)),
        ("Bottom", "bottom", b.inset_b.unwrap_or(3.6)),
    ] {
        ui.horizontal(|ui| {
            ui.add_sized(vec2(60.0, 20.0), egui::Label::new(l));
            if let Some(nv) = widgets::spinner(ui, &format!("m{k}"), v, 1.0, " pt", 60.0) {
                let _ = app.run("format.margins", json!({k: nv.max(0.0)}));
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label("Columns");
        for n in 1..=3 {
            if ui.small_button(n.to_string()).clicked() {
                let _ = app.run("format.columns", json!({"count": n, "spacing": 18}));
            }
        }
    });
}

fn picture(app: &mut SlideApp, ui: &mut Ui, sh: &deckcraft_model::Shape) {
    let adj = if let ShapeKind::Picture { fill } = &sh.kind { fill.adjust.clone() } else { Default::default() };
    let mut b = adj.brightness as f32;
    if ui.add(egui::Slider::new(&mut b, -1.0..=1.0).text("Brightness")).drag_stopped() {
        let _ = app.run("picture.adjust", json!({"brightness": b}));
    }
    let mut c = adj.contrast as f32;
    if ui.add(egui::Slider::new(&mut c, -1.0..=1.0).text("Contrast")).drag_stopped() {
        let _ = app.run("picture.adjust", json!({"contrast": c}));
    }
    let mut s = adj.saturation.unwrap_or(1.0) as f32;
    if ui.add(egui::Slider::new(&mut s, 0.0..=4.0).text("Saturation")).drag_stopped() {
        let _ = app.run("picture.adjust", json!({"saturation": s}));
    }
    let alpha = if let ShapeKind::Picture { fill } = &sh.kind { fill.alpha.unwrap_or(1.0) } else { 1.0 };
    let mut tr = (1.0 - alpha) as f32;
    if ui.add(egui::Slider::new(&mut tr, 0.0..=1.0).text("Transparency")).drag_stopped() {
        let _ = app.run("picture.adjust", json!({"transparency": tr}));
    }
    if ui.button("Reset Picture").clicked() {
        let _ = app.run("picture.reset", json!({}));
    }
}

fn fill_line(app: &mut SlideApp, ui: &mut Ui, sh: &deckcraft_model::Shape) {
    // Fill & Line.
    ui.label(egui::RichText::new("Fill").font(theme::bold(13.0)));
    let kind = match &sh.fill {
        None => "auto",
        Some(Fill::None) => "none",
        Some(Fill::Solid { .. }) => "solid",
        Some(Fill::Gradient(_)) => "gradient",
        Some(Fill::Picture(_)) => "picture",
        Some(Fill::Pattern(_)) => "pattern",
        _ => "auto",
    };
    for (l, k) in [("No fill", "none"), ("Solid fill", "solid"), ("Gradient fill", "gradient"), ("Pattern fill", "pattern"), ("Automatic", "auto")] {
        if ui.radio(kind == k, l).clicked() {
            match k {
                "none" => {
                    let _ = app.run("shape.fill", json!({"none": true}));
                }
                "solid" => {
                    let _ = app.run("shape.fill", json!({"color": "accent1"}));
                }
                "gradient" => {
                    let _ = app.run(
                        "shape.fill",
                        json!({"gradient": {"stops": [[0, "accent1"], [1, {"scheme": "accent1", "lumMod": 50000}]], "angle": 90}}),
                    );
                }
                "pattern" => {
                    let _ = app.run("shape.fill", json!({"pattern": {"preset": "pct25", "fg": "accent1", "bg": "bg1"}}));
                }
                _ => {
                    let _ = app.run("shape.fill", json!({"reset": true}));
                }
            }
        }
    }
    color_button(app, ui, "Color", Some("No Fill"), |c| match c {
        Some(c) => ("shape.fill".into(), json!({"color": cref_param(&c)})),
        None => ("shape.fill".into(), json!({"none": true})),
    });
    ui.horizontal(|ui| {
        ui.label("Transparency");
        let id = ui.id().with("ftr");
        let mut v: f32 = ui.data_mut(|d| d.get_temp(id).unwrap_or(0.0));
        let r = ui.add(egui::Slider::new(&mut v, 0.0..=100.0).suffix("%"));
        ui.data_mut(|d| d.insert_temp(id, v));
        if r.drag_stopped() || r.lost_focus() {
            let _ = app.run("shape.fill", json!({"transparency": v as f64 / 100.0}));
        }
    });
    if let Some(Fill::Gradient(g)) = &sh.fill {
        let sc = scheme(app);
        if let Some(ng) = crate::fillui::gradient_editor(ui, "shape-grad", g, &sc) {
            let _ = app.run("shape.fill", json!({"gradient": crate::fillui::gradient_json(&ng)}));
        }
        let mut rot = g.rotate_with_shape;
        if ui.checkbox(&mut rot, "Rotate with shape").changed() {
            let ng = deckcraft_model::style::Gradient { rotate_with_shape: rot, ..g.clone() };
            let _ = app.run("shape.fill", json!({"gradient": crate::fillui::gradient_json(&ng)}));
        }
    }
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Line").font(theme::bold(13.0)));
    let none = matches!(sh.line.as_ref().and_then(|l| l.fill.as_ref()), Some(Fill::None));
    if ui.radio(none, "No line").clicked() {
        let _ = app.run("shape.line", json!({"none": true}));
    }
    if ui.radio(!none && sh.line.is_some(), "Solid line").clicked() {
        let _ = app.run("shape.line", json!({"color": "tx1", "width": 1}));
    }
    if ui.radio(sh.line.is_none(), "Automatic").clicked() {
        let _ = app.run("shape.line", json!({"reset": true}));
    }
    color_button(app, ui, "Color", Some("No Outline"), |c| match c {
        Some(c) => ("shape.line".into(), json!({"color": cref_param(&c)})),
        None => ("shape.line".into(), json!({"none": true})),
    });
    ui.horizontal(|ui| {
        ui.label("Width");
        let w = sh.line.as_ref().and_then(|l| l.width).unwrap_or(0.75);
        if let Some(v) = widgets::spinner(ui, "lw", w, 0.25, " pt", 60.0) {
            let _ = app.run("shape.line", json!({"width": v.max(0.0)}));
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Dash");
        for (l, d) in [("—", "solid"), ("···", "sysDot"), ("- -", "dash"), ("-·-", "dashDot"), ("— —", "lgDash")] {
            if ui.small_button(l).clicked() {
                let _ = app.run("shape.line", json!({"dash": d}));
            }
        }
    });
}
