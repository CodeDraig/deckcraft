//! File picker, asynchronous delivery and save workflows.

use super::{SlideApp, ViewMode};
use serde_json::json;

impl SlideApp {
    pub(super) fn pick_and_open(&mut self) {
        if let Some(pick) = self.services.pick_open.as_mut() {
            if let Some(path) = pick("open") {
                let _ = self.open_path(&path);
            }
        } else if let Some(f) = self.services.open_async.as_mut() {
            f("open");
        }
    }

    pub fn open_path(&mut self, path: &str) -> Result<(), String> {
        self.session.execute("file.open", &json!({"path": path})).map_err(|e| e.to_string())?;
        self.ui.recent.retain(|p| p != path);
        self.ui.recent.insert(0, path.to_string());
        self.ui.recent.truncate(12);
        self.ui.view = ViewMode::Normal;
        Ok(())
    }

    pub(super) fn pick_and_insert(&mut self, purpose: &str, cmd: &str) {
        if let Some(pick) = self.services.pick_open.as_mut() {
            if let Some(path) = pick(purpose) {
                let r = match self.services.read.as_ref().map(|r| r(&path)) {
                    Some(Ok(bytes)) => {
                        let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        self.session.execute(cmd, &json!({"name": name, "data": deckcraft_engine::cmd::base64_encode(&bytes)}))
                    }
                    _ => self.session.execute(cmd, &json!({"path": path})),
                };
                if let Err(e) = r {
                    self.set_status(e.to_string());
                }
            }
        } else if let Some(f) = self.services.open_async.as_mut() {
            f(purpose);
        }
    }

    pub fn save_as_dialog(&mut self, format: &str) {
        let name = self.session.active().map(|d| d.title()).unwrap_or_else(|| "Presentation".into());
        let ext = if format == "pptx" { "pptx" } else { "deckcraft" };
        let suggested = format!("{name}.{ext}");
        if let Some(pick) = self.services.pick_save.as_mut() {
            if let Some(path) = pick(&suggested) {
                match self.session.execute("file.saveAs", &json!({"path": path})) {
                    Ok(_) => {
                        self.set_status(format!("Saved {path}"));
                        self.ui.recent.retain(|p| *p != path);
                        self.ui.recent.insert(0, path);
                    }
                    Err(e) => self.set_status(e.to_string()),
                }
            }
        } else if let Some(dl) = self.services.download.as_mut()
            && let Some(d) = self.session.active()
        {
            match deckcraft_engine::cmd::file::save_bytes(&d.doc, ext) {
                Ok(bytes) => dl(&suggested, &bytes),
                Err(e) => self.set_status(e.to_string()),
            }
        }
    }

    /// Save every presentation with unsaved changes (asking for a location where needed).
    /// Returns false if one wasn't saved (cancelled or failed).
    pub fn save_all(&mut self) -> bool {
        let dirty: Vec<usize> = self.session.documents().iter().enumerate().filter(|(_, d)| d.is_dirty()).map(|(i, _)| i).collect();
        for i in dirty {
            self.session.set_active(i);
            self.save();
            if self.session.documents().get(i).is_some_and(|d| d.is_dirty()) {
                return false;
            }
        }
        true
    }

    pub fn save(&mut self) {
        if self.session.active().and_then(|d| d.path.clone()).is_some() {
            match self.session.execute("file.save", &json!({})) {
                Ok(_) => self.set_status("Saved"),
                Err(e) => self.set_status(e.to_string()),
            }
        } else {
            self.save_as_dialog("deckcraft");
        }
    }

    pub(super) fn drain_inbox(&mut self) {
        let Some(inbox) = self.services.inbox.clone() else { return };
        let files: Vec<(String, Vec<u8>)> = inbox.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default();
        for (name, bytes) in files {
            self.open_or_insert_bytes(&name, &bytes);
        }
    }

    /// A dropped or picked file: presentations open, media is inserted.
    pub fn open_or_insert_bytes(&mut self, name: &str, bytes: &[u8]) {
        let lower = name.to_ascii_lowercase();
        let data = deckcraft_engine::cmd::base64_encode(bytes);
        let ct = deckcraft_engine::cmd::insert::content_type(name, bytes);
        let cmd = if lower.ends_with(".deckcraft") || lower.ends_with(".pptx") || lower.ends_with(".potx") || lower.ends_with(".ppsx") {
            "file.openBytes"
        } else if ct.starts_with("image/") {
            "insert.picture"
        } else if ct.starts_with("audio/") {
            "insert.audio"
        } else if ct.starts_with("video/") {
            "insert.video"
        } else if lower.ends_with(".txt") || lower.ends_with(".md") {
            "slide.fromOutline"
        } else {
            self.set_status(format!("{name}: DeckCraft can't open this kind of file"));
            return;
        };
        let p = if cmd == "slide.fromOutline" { json!({"text": String::from_utf8_lossy(bytes)}) } else { json!({"name": name, "data": data}) };
        if self.session.active().is_none() && cmd != "file.openBytes" {
            let _ = self.session.execute("file.new", &json!({}));
        }
        if let Err(e) = self.session.execute(cmd, &p) {
            self.set_status(e.to_string());
        }
    }
}
