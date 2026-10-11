//! Presentation playback, recording and workspace views.

use super::big;
use crate::SlideApp;
use crate::icons::Icon;
use crate::theme::{self};
use crate::widgets::{big_button, big_button_ex, group, rows};
use deckcraft_engine::Target;
use egui::Ui;
use serde_json::json;

pub(super) fn slide_show(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        if big_button(ui, Icon::PlayFromStart, "Play from\nStart", true).clicked() {
            app.start_show(0, false);
        }
        if big_button(ui, Icon::PlayFromCurrent, "Play from\nCurrent Slide", true).clicked() {
            let i = app.session.active().map(|d| d.selection.slide).unwrap_or(0);
            app.start_show(i, false);
        }
        let on = app.ui.presenter_view;
        if big_button_ex(ui, Icon::PresenterView, "Presenter\nView", true, false, on).0.clicked() {
            app.ui.presenter_view = !on;
        }
        big(app, ui, Icon::CustomShow, "Custom\nShow", "app.dialog", json!({"id": "customShow"}));
    });
    group(ui, |ui| {
        big(app, ui, Icon::SetUpShow, "Set Up\nSlide Show", "app.dialog", json!({"id": "setupShow"}));
        let hidden = app.session.active().and_then(|d| d.current_slide().map(|s| s.hidden)).unwrap_or(false);
        if big_button_ex(ui, Icon::HideSlide, "Hide\nSlide", true, false, hidden).0.clicked() {
            let _ = app.run("slide.hide", json!({}));
        }
    });
    group(ui, |ui| {
        if big_button(ui, Icon::RehearseTimings, "Rehearse\nTimings", true).clicked() {
            app.start_show(0, false);
            if let Some(s) = app.show.as_mut() {
                s.rehearse = true;
            }
        }
        let r = big_button(ui, Icon::Record, "Record", false);
        let _ = r.on_hover_text("Recording narration needs audio capture (coming soon)");
    });
    group(ui, |ui| {
        rows(ui, |ui| {
            let mut use_t = app.session.active().is_none_or(|d| d.doc.show.use_timings);
            if ui.checkbox(&mut use_t, "Use Timings").changed() {
                let _ = app.run("show.setup", json!({"useTimings": use_t}));
            }
            let mut narr = app.session.active().is_none_or(|d| !d.doc.show.without_narration);
            if ui.checkbox(&mut narr, "Play Narrations").changed() {
                let _ = app.run("show.setup", json!({"noNarration": !narr}));
            }
            let mut mc = app.session.active().is_none_or(|d| d.doc.show.show_media_controls);
            let _ = ui.checkbox(&mut mc, "Show Media Controls");
        });
    });
}

pub(super) fn record(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        let r = big_button(ui, Icon::Cameo, "Cameo", false);
        let _ = r.on_hover_text("Camera capture is not available yet");
        if big_button(ui, Icon::PlayFromStart, "From\nBeginning", true).clicked() {
            app.start_show(0, false);
            if let Some(s) = app.show.as_mut() {
                s.rehearse = true;
            }
        }
        if big_button(ui, Icon::PlayFromCurrent, "From\nCurrent Slide", true).clicked() {
            let i = app.session.active().map(|d| d.selection.slide).unwrap_or(0);
            app.start_show(i, false);
            if let Some(s) = app.show.as_mut() {
                s.rehearse = true;
            }
        }
    });
    group(ui, |ui| {
        if big_button(ui, Icon::Help, "Learn\nMore", true).clicked() {
            app.open_url(deckcraft_engine::links::APP_PAGE);
        }
    });
}

pub(super) fn view(app: &mut SlideApp, ui: &mut Ui) {
    use crate::ViewMode;
    group(ui, |ui| {
        for (icon, label, id, mode) in [
            (Icon::ViewNormal, "Normal", "view.normal", Some(ViewMode::Normal)),
            (Icon::ViewOutline, "Outline\nView", "view.outline", Some(ViewMode::Outline)),
            (Icon::ViewSorter, "Slide\nSorter", "view.sorter", Some(ViewMode::Sorter)),
            (Icon::ViewNotesPage, "Notes\nPage", "view.notesPage", Some(ViewMode::NotesPage)),
            (Icon::ViewReading, "Reading\nView", "view.reading", None),
        ] {
            let sel = mode.is_some_and(|m| app.ui.view == m);
            if big_button_ex(ui, icon, label, true, false, sel).0.clicked() {
                let _ = app.run(id, json!({}));
            }
        }
    });
    group(ui, |ui| {
        let in_master = app.session.active().is_some_and(|d| d.selection.target != Target::Slides);
        if big_button_ex(ui, Icon::SlideMaster, "Slide\nMaster", true, false, in_master).0.clicked() {
            let _ = app.run(if in_master { "view.closeMaster" } else { "view.slideMaster" }, json!({}));
        }
        let r = big_button(ui, Icon::HandoutMaster, "Handout\nMaster", false);
        let _ = r.on_hover_text("Handout Master (coming soon)");
        let r = big_button(ui, Icon::NotesMaster, "Notes\nMaster", false);
        let _ = r.on_hover_text("Notes Master (coming soon)");
    });
    group(ui, |ui| {
        rows(ui, |ui| {
            let mut r = app.ui.ruler;
            if ui.checkbox(&mut r, "Ruler").changed() {
                app.ui.ruler = r;
            }
            let mut g = app.ui.gridlines;
            if ui.checkbox(&mut g, "Gridlines").changed() {
                app.ui.gridlines = g;
            }
            let mut gd = app.ui.guides;
            if ui.checkbox(&mut gd, "Guides").changed() {
                app.ui.guides = gd;
            }
        });
        let n = app.ui.notes;
        if big_button_ex(ui, Icon::Notes, "Notes", true, false, n).0.clicked() {
            app.ui.notes = !n;
        }
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::ZoomGlass, "Zoom", true);
        egui::Popup::menu(&r).show(|ui| {
            for pct in [400, 200, 150, 100, 75, 66, 50, 33] {
                if ui.button(format!("{pct}%")).clicked() {
                    let _ = app.run("view.zoom", json!({"percent": pct}));
                }
            }
        });
        if big_button(ui, Icon::FitToWindow, "Fit to\nWindow", true).clicked() {
            app.ui.zoom = None;
        }
    });
    group(ui, |ui| {
        let gs = app.ui.grayscale;
        if big_button_ex(ui, Icon::Grayscale, "Grayscale", true, false, gs).0.clicked() {
            app.ui.grayscale = !gs;
        }
        let dark = app.ui.brightness == theme::Brightness::Dark;
        if big_button_ex(ui, Icon::Palette, "Dark\nMode", true, false, dark).0.clicked() {
            let _ = app.run("view.dark", json!({}));
        }
    });
}
