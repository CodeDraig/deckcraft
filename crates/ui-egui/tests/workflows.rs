//! UI workflow boundaries: dialog dispatch and failed/successful browser exports.

use std::sync::{Arc, Mutex};

use deckcraft_engine::Session;
use deckcraft_ui_egui::{Services, SlideApp, dialogs};
use serde_json::json;

fn frame(app: &mut SlideApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input =
        egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440.0, 900.0))), events, ..Default::default() };
    ctx.run_ui(input, |ui| {
        app.logic(ui.ctx());
        app.ui(ui);
    })
    .drop_without_applying_deltas();
}

fn enter() -> egui::Event {
    egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }
}

#[test]
fn dialog_views_preserve_the_document_until_submission() {
    let mut app = SlideApp::new(Session::with_new(), Services::default());
    app.run("shape.insert", json!({"preset": "rect", "rect": [10, 10, 100, 100]})).unwrap();
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let document = app.session.active().unwrap().doc.clone();
    for id in [
        "table",
        "slideSize",
        "headerFooter",
        "hyperlink",
        "setupShow",
        "zoom",
        "comment",
        "altText",
        "outline",
        "renameSection",
        "renameShape",
        "renameLayout",
        "paragraph",
        "about",
        "preferences",
        "export",
        "symbol",
        "equation",
        "spelling",
        "accessibility",
        "language",
        "customShow",
        "chartData",
        "trim",
        "smartart",
        "quit",
    ] {
        app.run("app.dialog", json!({"id": id})).unwrap();
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.dialog.as_ref().map(|d| d.id.as_str()), Some(id));
        assert!(Arc::ptr_eq(&document, &app.session.active().unwrap().doc), "{id} mutated the document");
    }
}

#[test]
fn browser_export_delivers_modern_threads_and_reports_failures_without_downloading() {
    for invalid in [false, true] {
        let downloads = Arc::new(Mutex::new(Vec::<(String, Vec<u8>)>::new()));
        let sink = downloads.clone();
        let services =
            Services { download: Some(Box::new(move |name, bytes| sink.lock().unwrap().push((name.into(), bytes.into())))), ..Default::default() };
        let mut app = SlideApp::new(Session::with_new(), services);
        app.run("comment.add", json!({"text": "Root"})).unwrap();
        app.run("comment.reply", json!({"index": 0, "text": "Reply"})).unwrap();
        if invalid {
            let doc = Arc::make_mut(&mut app.session.active_mut().unwrap().doc);
            Arc::make_mut(&mut doc.slides[0]).comments[0].date.clear();
        }
        let document = app.session.active().unwrap().doc.clone();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![]);
        let mut dialog = dialogs::Dialog::new("export");
        dialog.fields.insert("fmt".into(), "pptx".into());
        app.dialog = Some(dialog);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![enter()]);
        let downloads = downloads.lock().unwrap();
        if invalid {
            assert!(downloads.is_empty());
            assert!(app.status.as_ref().unwrap().0.contains("creation date"));
        } else {
            assert_eq!(downloads.len(), 1);
            let (name, bytes) = &downloads[0];
            assert!(name.ends_with(".pptx"));
            let imported = deckcraft_engine::cmd::file::open_presentation(name, bytes).unwrap();
            assert_eq!(imported.slides[0].comments, document.slides[0].comments);
        }
        assert!(Arc::ptr_eq(&document, &app.session.active().unwrap().doc));
    }
}
