//! Title bar, ribbon tabs and the ribbon card with every tab's groups.

mod master;
use master::slide_master;

mod playback;
use playback::playback;

mod chart;
use chart::chart_design;

mod table;
pub use table::paint_table_style;
use table::{table_design, table_layout};

mod picture;
use picture::picture_format;

mod presentation;
use presentation::{record, slide_show, view};

mod animations;
use animations::animations;
pub use animations::paint_anim_tile;

mod transitions;
pub use transitions::paint_transition_tile;
use transitions::transitions;

mod design;
use design::design;
pub use design::paint_theme_tile;

mod insert;
use insert::insert;
pub use insert::table_grid_picker;

mod arrange;
pub use arrange::{align_menu, arrange_menu, menu_item};
use arrange::{arrange_group, size_group};

mod appearance;
use appearance::{drawing_group, shape_format};
pub use appearance::{effects_menu, outline_menu};

mod shapes;
use shapes::draw;
pub use shapes::{paint_preset, shapes_gallery};

mod home;
use home::home;

mod text;
pub use text::font_families;

mod clipboard;

mod chrome;
pub use chrome::{TITLE_H, title_bar};

use deckcraft_engine::Target;
use deckcraft_model::ShapeKind;
use egui::{Align2, CornerRadius, Rect, Sense, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::SlideApp;
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::widgets::{RIBBON_H, big_button, big_button_ex, group, icon_toggle, small_button};

/// Is engine command `id` enabled right now?
pub fn enabled(app: &SlideApp, id: &str) -> bool {
    match deckcraft_engine::find_command(id) {
        Some(c) => (c.enabled)(&app.session).is_ok(),
        None => crate::UI_COMMANDS.iter().any(|c| c.0 == id),
    }
}

/// Which contextual tabs apply to the selection.
fn contextual(app: &SlideApp) -> Vec<&'static str> {
    let mut v = vec![];
    let Some(st) = app.session.active() else { return v };
    if st.selection.target != Target::Slides {
        v.push("Slide Master");
    }
    let shapes = st.selected_shapes();
    if shapes.iter().any(|s| matches!(s.kind, ShapeKind::Shape | ShapeKind::Group { .. } | ShapeKind::Connector { .. }))
        || st.selection.text.is_some()
    {
        v.push("Shape Format");
    }
    if shapes.iter().any(|s| matches!(s.kind, ShapeKind::Picture { .. })) {
        v.push("Picture Format");
    }
    if shapes.iter().any(|s| matches!(s.kind, ShapeKind::Table(_))) || st.selection.text.as_ref().is_some_and(|t| t.cell.is_some()) {
        v.push("Table Design");
        v.push("Layout");
    }
    if shapes.iter().any(|s| matches!(s.kind, ShapeKind::Chart(_))) {
        v.push("Chart Design");
    }
    if shapes.iter().any(|s| matches!(s.kind, ShapeKind::Media(_))) {
        v.push("Playback");
    }
    v
}

const TABS: [&str; 10] = ["Home", "Insert", "Draw", "Design", "Transitions", "Animations", "Slide Show", "Record", "Review", "View"];

/// Tab row and the ribbon card.
pub fn show(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx_tabs = contextual(app);
    if !TABS.contains(&app.ui.tab.as_str()) && !ctx_tabs.contains(&app.ui.tab.as_str()) {
        app.ui.tab = "Home".into();
    }
    let h = 28.0 + if app.ui.ribbon_collapsed { 4.0 } else { RIBBON_H + 20.0 };
    egui::Panel::top("ribbon").exact_size(h).frame(egui::Frame::NONE.fill(t.chrome)).show(ui, |ui| {
        let rect = ui.max_rect();
        // Tabs.
        let mut x = rect.min.x + 12.0;
        let y = rect.min.y + 2.0;
        let all: Vec<(&str, bool)> = TABS.iter().map(|s| (*s, false)).chain(ctx_tabs.iter().map(|s| (*s, true))).collect();
        for (name, contextual) in all {
            let font = theme::font(13.0);
            let w = ui.fonts_mut(|f| f.layout_no_wrap(name.to_string(), font.clone(), t.text).size().x) + 18.0;
            let r = Rect::from_min_size(pos2(x, y), vec2(w, 24.0));
            let resp = ui.interact(r, ui.id().with(("tab", name)), Sense::click());
            let active = app.ui.tab == name;
            if resp.hovered() && !active {
                ui.painter().rect_filled(r.shrink2(vec2(2.0, 2.0)), CornerRadius::same(4), t.hover);
            }
            let col = if contextual { t.contextual } else { t.text };
            let f = if active { theme::bold(13.0) } else { font };
            ui.painter().text(r.center(), Align2::CENTER_CENTER, name, f, col);
            if active {
                let uw = (w - 20.0).max(16.0);
                ui.painter().rect_filled(Rect::from_center_size(pos2(r.center().x, r.max.y - 1.0), vec2(uw, 3.0)), CornerRadius::same(1), t.accent);
            }
            if resp.clicked() {
                if active {
                    app.ui.ribbon_collapsed = !app.ui.ribbon_collapsed;
                } else {
                    app.ui.tab = name.to_string();
                    app.ui.ribbon_collapsed = false;
                }
            }
            if resp.double_clicked() {
                app.ui.ribbon_collapsed = !app.ui.ribbon_collapsed;
            }
            x += w + 2.0;
        }
        if app.ui.ribbon_collapsed {
            return;
        }
        // The white card.
        let card = Rect::from_min_max(pos2(rect.min.x + 8.0, rect.min.y + 30.0), pos2(rect.max.x - 8.0, rect.min.y + 30.0 + RIBBON_H + 10.0));
        ui.painter().rect_filled(card.translate(vec2(0.0, 1.0)), CornerRadius::same(10), t.card_border);
        ui.painter().rect_filled(card, CornerRadius::same(10), t.card);
        let inner = Rect::from_min_size(pos2(card.min.x + 10.0, card.min.y + 5.0), vec2(card.width() - 20.0, RIBBON_H));
        let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Min)));
        cui.set_clip_rect(inner.expand2(vec2(0.0, 3.0)));
        cui.spacing_mut().item_spacing.x = 1.0;
        let ui = &mut cui;
        let tab = app.ui.tab.clone();
        match tab.as_str() {
            "Home" => home(app, ui),
            "Insert" => insert(app, ui),
            "Draw" => draw(app, ui),
            "Design" => design(app, ui),
            "Transitions" => transitions(app, ui),
            "Animations" => animations(app, ui),
            "Slide Show" => slide_show(app, ui),
            "Record" => record(app, ui),
            "Review" => review(app, ui),
            "View" => view(app, ui),
            "Shape Format" => shape_format(app, ui),
            "Picture Format" => picture_format(app, ui),
            "Table Design" => table_design(app, ui),
            "Layout" => table_layout(app, ui),
            "Chart Design" => chart_design(app, ui),
            "Playback" => playback(app, ui),
            "Slide Master" => slide_master(app, ui),
            _ => {}
        }
    });
}

fn big(app: &mut SlideApp, ui: &mut Ui, icon: Icon, label: &str, id: &str, p: Value) {
    let en = enabled(app, id);
    if big_button(ui, icon, label, en).on_hover_text(tip(id, label)).clicked() {
        let _ = app.run(id, p);
    }
}

fn small(app: &mut SlideApp, ui: &mut Ui, icon: Icon, label: &str, id: &str, p: Value) {
    let en = enabled(app, id);
    if small_button(ui, icon, label, en, false).on_hover_text(tip(id, label)).clicked() {
        let _ = app.run(id, p);
    }
}

fn toggle(app: &mut SlideApp, ui: &mut Ui, icon: Icon, label: &str, id: &str, on: bool) {
    let en = enabled(app, id);
    if icon_toggle(ui, icon, &tip(id, label), en, on).clicked() {
        let _ = app.run(id, json!({}));
    }
}

fn tip(id: &str, label: &str) -> String {
    let sc = deckcraft_engine::find_command(id).and_then(|c| c.shortcut).or_else(|| crate::UI_COMMANDS.iter().find(|c| c.0 == id).and_then(|c| c.2));
    match sc {
        Some(s) => format!("{label} ({})", pretty_shortcut(s)),
        None => label.to_string(),
    }
}

pub fn pretty_shortcut(s: &str) -> String {
    if cfg!(target_os = "macos") {
        s.replace("Cmd+", "⌘").replace("Shift+", "⇧").replace("Alt+", "⌥").replace("Ctrl+", "⌃")
    } else {
        s.replace("Cmd+", "Ctrl+")
    }
}

fn scheme(app: &SlideApp) -> deckcraft_color::ColorScheme {
    app.session.active().and_then(|d| d.doc.masters.first().map(|m| m.scheme())).unwrap_or_else(|| deckcraft_model::Theme::default().colors)
}

/// The formatting state at the selection (bold, size…).
pub fn fmt_state(app: &mut SlideApp) -> Value {
    if app.session.active().is_none_or(|d| d.selection.shapes.is_empty() && d.selection.text.is_none()) {
        return json!({});
    }
    app.session.execute("format.state", &json!({})).unwrap_or_default()
}

// ---------- Home ----------

/// A ColorRef as a command colour parameter.
pub fn cref_param(c: &deckcraft_model::ColorRef) -> Value {
    match &c.base {
        deckcraft_model::ColorBase::Scheme { slot } => {
            let mut o = serde_json::Map::new();
            o.insert("scheme".into(), json!(slot.xml_name()));
            for m in &c.mods {
                if let Some(v) = m.value() {
                    o.insert(m.xml_name().into(), json!(v));
                }
            }
            Value::Object(o)
        }
        deckcraft_model::ColorBase::Rgb { rgb } if c.mods.is_empty() => json!(format!("#{}", rgb.hex())),
        deckcraft_model::ColorBase::Rgb { rgb } => {
            let mut o = serde_json::Map::new();
            o.insert("rgb".into(), json!(format!("#{}", rgb.hex())));
            for m in &c.mods {
                if let Some(v) = m.value() {
                    o.insert(m.xml_name().into(), json!(v));
                }
            }
            Value::Object(o)
        }
        _ => json!("#000000"),
    }
}

// ---------- Insert ----------

// ---------- Draw ----------

// ---------- Design ----------

// ---------- Transitions ----------

// ---------- Animations ----------

// ---------- Slide Show / Record / Review / View ----------

fn review(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        big(app, ui, Icon::Spelling, "Spelling", "app.dialog", json!({"id": "spelling"}));
        let r = big_button(ui, Icon::Thesaurus, "Thesaurus", false);
        let _ = r.on_hover_text("Thesaurus (coming soon)");
    });
    group(ui, |ui| {
        big(app, ui, Icon::Accessibility, "Check\nAccessibility", "app.dialog", json!({"id": "accessibility"}));
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::Translate, "Translate", false);
        let _ = r.on_hover_text("Translation needs a language model (not bundled)");
        big(app, ui, Icon::Language, "Language", "app.dialog", json!({"id": "language"}));
    });
    group(ui, |ui| {
        big(app, ui, Icon::NewComment, "New\nComment", "app.dialog", json!({"id": "comment"}));
        big(app, ui, Icon::DeleteComment, "Delete", "comment.delete", json!({}));
        let show = app.ui.pane.as_deref() == Some("comments");
        if big_button_ex(ui, Icon::ShowComments, "Show\nComments", true, false, show).0.clicked() {
            let _ = app.run("view.pane", json!({"pane": "comments", "toggle": true}));
        }
    });
}

// ---------- contextual tabs ----------
