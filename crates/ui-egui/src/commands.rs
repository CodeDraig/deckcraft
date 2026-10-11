//! Dispatch commands and consume their UI requests.

use super::{SlideApp, UI_COMMANDS, ViewMode, dialogs, media, show, theme};
use deckcraft_engine::UiRequest;
use serde_json::{Value, json};

impl SlideApp {
    /// Run a command by id (UI commands here, everything else in the engine). Errors show in the
    /// status bar and are returned.
    pub fn run(&mut self, id: &str, params: Value) -> Result<Value, String> {
        let r = if UI_COMMANDS.iter().any(|c| c.0 == id) {
            self.run_ui(id, &params)
        } else {
            self.session.execute(id, &params).map_err(|e| e.to_string())
        };
        if let Err(e) = &r {
            self.set_status(e.clone());
        }
        self.drain_requests();
        r
    }

    pub(super) fn flag(&mut self, params: &Value, cur: bool) -> bool {
        params.get("on").and_then(Value::as_bool).unwrap_or(!cur)
    }

    pub(super) fn run_ui(&mut self, id: &str, p: &Value) -> Result<Value, String> {
        match id {
            "view.normal" => self.ui.view = ViewMode::Normal,
            "view.outline" => self.ui.view = ViewMode::Outline,
            "view.sorter" => self.ui.view = ViewMode::Sorter,
            "view.notesPage" => self.ui.view = ViewMode::NotesPage,
            "view.reading" => self.start_show(self.session.active().map(|d| d.selection.slide).unwrap_or(0), true),
            "view.notes" => self.ui.notes = self.flag(p, self.ui.notes),
            "view.ruler" => self.ui.ruler = self.flag(p, self.ui.ruler),
            "view.gridlines" => self.ui.gridlines = self.flag(p, self.ui.gridlines),
            "view.guides" => self.ui.guides = self.flag(p, self.ui.guides),
            "view.zoomIn" | "view.zoomOut" => {
                let cur = self.ui.zoom.unwrap_or(self.canvas_scale);
                let steps = [0.1, 0.25, 0.33, 0.5, 0.66, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];
                let next = if id == "view.zoomIn" {
                    steps.iter().copied().find(|s| *s > cur + 0.01).unwrap_or(4.0)
                } else {
                    steps.iter().rev().copied().find(|s| *s < cur - 0.01).unwrap_or(0.1)
                };
                self.ui.zoom = Some(next);
            }
            "view.zoom" => {
                let pct = p.get("percent").and_then(Value::as_f64).ok_or("missing `percent`")?;
                self.ui.zoom = Some((pct as f32 / 100.0).clamp(0.1, 4.0));
            }
            "view.fit" => self.ui.zoom = None,
            "view.pane" => {
                let pane = p.get("pane").and_then(Value::as_str).map(String::from);
                self.ui.pane =
                    if pane.is_some() && pane == self.ui.pane && p.get("toggle").and_then(Value::as_bool).unwrap_or(false) { None } else { pane };
                if let Some(tab) = p.get("tab").and_then(Value::as_str) {
                    self.ui.format_tab = tab.to_string();
                }
            }
            "view.tab" => {
                let tab = p.get("tab").and_then(Value::as_str).ok_or("missing `tab`")?;
                self.ui.tab = tab.to_string();
                self.ui.ribbon_collapsed = false;
            }
            "view.collapseRibbon" => self.ui.ribbon_collapsed = !self.ui.ribbon_collapsed,
            "view.dark" => {
                let on = self.flag(p, self.ui.brightness == theme::Brightness::Dark);
                self.ui.brightness = if on { theme::Brightness::Dark } else { theme::Brightness::Light };
                self.restyle = true;
            }
            "view.grayscale" => self.ui.grayscale = self.flag(p, self.ui.grayscale),
            "show.start" => {
                let from = p.get("from").and_then(Value::as_u64).map(|v| v as usize).unwrap_or(0);
                self.start_show(from, false);
            }
            "show.presenter" => self.ui.presenter_view = self.flag(p, self.ui.presenter_view),
            "show.end" => {
                self.show = None;
                self.media.stop_owned(media::Owner::Show);
            }
            "app.about" => self.dialog = Some(dialogs::Dialog::new("about")),
            "app.palette" => self.palette = Some((String::new(), 0)),
            "app.preferences" => self.dialog = Some(dialogs::Dialog::new("preferences")),
            "app.openDialog" => self.pick_and_open(),
            "app.insertPictureDialog" => self.pick_and_insert("picture", "insert.picture"),
            "app.insertAudioDialog" => self.pick_and_insert("audio", "insert.audio"),
            "app.insertVideoDialog" => self.pick_and_insert("video", "insert.video"),
            "app.saveAsDialog" => self.save_as_dialog("deckcraft"),
            "app.exportDialog" => self.dialog = Some(dialogs::Dialog::new("export")),
            "app.dialog" => {
                let d = p.get("id").and_then(Value::as_str).ok_or("missing `id`")?;
                self.dialog = Some(dialogs::Dialog::new(d));
            }
            "app.links" => {
                use deckcraft_engine::links::*;
                return Ok(json!({"discord": DISCORD, "website": WEBSITE, "apps": APPS, "appPage": APP_PAGE, "github": GITHUB}));
            }
            _ => return Err(format!("unknown UI command `{id}`")),
        }
        Ok(Value::Null)
    }

    pub fn start_show(&mut self, from: usize, reading: bool) {
        self.media.stop_owned(media::Owner::Editor);
        if let Some(d) = self.session.active() {
            self.show = Some(show::Show::new(&d.doc, from, reading, self.ui.presenter_view && !reading));
        }
    }

    /// Act on requests commands left for the UI.
    pub(super) fn drain_requests(&mut self) {
        for r in std::mem::take(&mut self.session.ui_requests) {
            match r {
                UiRequest::Dialog { id, params } => {
                    let mut d = dialogs::Dialog::new(&id);
                    d.params = params;
                    self.dialog = Some(d);
                }
                UiRequest::Pane { id } => self.ui.pane = Some(id),
                UiRequest::StartShow { from } => self.start_show(from, false),
                UiRequest::Message { text } => self.set_status(text),
                UiRequest::PickFile { purpose } => {
                    if purpose == "saveAs" {
                        self.save_as_dialog("deckcraft");
                    }
                }
                UiRequest::Media { action, shape, ms } => {
                    let owner = if self.show.is_some() { media::Owner::Show } else { media::Owner::Editor };
                    if let Some(doc) = self.session.active().map(|d| d.doc.clone()) {
                        self.media.request(&doc, &action, shape, ms, owner);
                    }
                }
            }
        }
    }
}
