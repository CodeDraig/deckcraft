//! The DeckCraft engine façade.
//!
//! Every user-visible action is a command with a stable id (`slide.new`, `shape.insert`,
//! `text.insert`, `format.bold`…) and JSON parameters. The egui UI, the CLI, the control channel
//! and MCP all go through [`Session::execute`]. Pointer tools reduce to commands too, so every
//! gesture is journaled, undoable and replayable.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod document;
use document::clamp_selection;
pub(crate) use document::push_undo;
pub use document::{DocState, History, HistoryEntry, Interaction, Selection, Target, TextSel, shapes_mut};

pub mod cmd;
pub mod connect;
pub mod guard;
pub mod keys;
pub mod links;
pub mod recovery;
pub mod sample;
pub mod tools;

use std::sync::Arc;

use deckcraft_model::{Presentation, Shape, ShapeId, Slide};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use cmd::{CommandInfo, CommandSpec, command_specs, find_command};
pub use deckcraft_geom as geom;
pub use deckcraft_media as media;
pub use deckcraft_model as model;
pub use deckcraft_render as render;
pub use tools::{Mods, PointerEvent, PointerKind, ToolKind};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("command `{0}` is not available right now: {1}")]
    Disabled(String, String),
    #[error("invalid parameters for `{cmd}`: {msg}")]
    BadParams { cmd: String, msg: String },
    #[error("no active presentation")]
    NoDocument,
    #[error("{0}")]
    Other(String),
    #[error("internal error in `{0}` (the presentation was kept as it was): {1}")]
    Internal(String, String),
}

impl From<deckcraft_model::ModelError> for EngineError {
    fn from(e: deckcraft_model::ModelError) -> Self {
        EngineError::Other(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, EngineError>;

/// Engine preferences (the UI keeps its own).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prefs {
    pub snap_to_grid: bool,
    pub grid_spacing: f64,
    pub smart_guides: bool,
    /// Ctrl+D / Paste offset.
    pub duplicate_offset: f64,
    pub autocorrect: bool,
    pub smart_quotes: bool,
    pub author: String,
    pub undo_limit: usize,
    /// Minutes between AutoRecover saves of unsaved presentations.
    pub recovery_minutes: f64,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            snap_to_grid: true,
            grid_spacing: 6.0,
            smart_guides: true,
            duplicate_offset: 12.0,
            autocorrect: true,
            smart_quotes: true,
            author: "Presenter".into(),
            undo_limit: 500,
            recovery_minutes: 1.0,
        }
    }
}

/// Things a command or tool asks the UI to do (open a dialog, show a pane, start the show…).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum UiRequest {
    Dialog {
        id: String,
        params: Value,
    },
    Pane {
        id: String,
    },
    StartShow {
        from: usize,
    },
    Message {
        text: String,
    },
    PickFile {
        purpose: String,
    },
    /// Media playback for the UI host (`media.play` / `pause` / `stop` / `seek`): `action` is
    /// `play`, `pause`, `toggle`, `stop` or `seek`; `ms` the media time to start from / seek to.
    Media {
        action: String,
        shape: ShapeId,
        ms: Option<u64>,
    },
}

/// Playback state of a media shape as the UI host reports it (see [`Session::media_status`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaStatus {
    /// `loading`, `playing`, `paused` or `ended`.
    pub state: String,
    pub position_ms: u64,
    pub duration_ms: u64,
    /// Why the media is silent / blank, when it can't be decoded.
    pub error: Option<String>,
}

/// Clipboard contents copied within the app.
#[derive(Clone, Debug, Default)]
pub struct Clipboard {
    pub shapes: Vec<Shape>,
    pub slides: Vec<Slide>,
    /// Media the copied shapes/slides reference.
    pub media: Vec<deckcraft_model::MediaItem>,
    pub text: Option<deckcraft_model::TextBody>,
    pub plain: String,
    /// How many times the shapes were pasted (each paste offsets further).
    pub pastes: u32,
}

pub struct Session {
    docs: Vec<DocState>,
    active: Option<usize>,
    pub prefs: Prefs,
    pub journal: Vec<(String, Value)>,
    pub clipboard: Clipboard,
    pub ui_requests: Vec<UiRequest>,
    pub tool: tools::ToolState,
    pub(crate) untitled: u32,
    /// Format Painter: copied shape/text formatting and whether it stays on (double-click).
    pub painter: Option<(cmd::format::Painted, bool)>,
    /// Set as Default Shape: the look new shapes get.
    pub default_look: Option<cmd::format::Painted>,
    /// Nesting of commands run by commands: only the outermost one is an undo step and a journal
    /// entry.
    depth: u32,
    /// Crash-recovery folder (the app sets it; saving or closing a presentation removes its entry).
    pub recovery_dir: Option<std::path::PathBuf>,
    /// Media playback state, written by the UI host every frame (empty without a host).
    pub media_status: std::collections::HashMap<ShapeId, MediaStatus>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        guard::install_panic_hook();
        Session {
            docs: vec![],
            active: None,
            prefs: Prefs::default(),
            journal: vec![],
            clipboard: Clipboard::default(),
            ui_requests: vec![],
            tool: tools::ToolState::default(),
            untitled: 0,
            default_look: None,
            painter: None,
            depth: 0,
            recovery_dir: None,
            media_status: Default::default(),
        }
    }
    /// A session with one new presentation open.
    pub fn with_new() -> Self {
        let mut s = Self::new();
        let _ = s.execute("file.new", &Value::Null);
        s
    }
    pub fn documents(&self) -> &[DocState] {
        &self.docs
    }
    pub fn active_index(&self) -> Option<usize> {
        self.active
    }
    pub fn active(&self) -> Option<&DocState> {
        self.active.and_then(|i| self.docs.get(i))
    }
    pub fn active_mut(&mut self) -> Option<&mut DocState> {
        self.active.and_then(|i| self.docs.get_mut(i))
    }
    pub fn doc(&self) -> Result<&DocState> {
        self.active().ok_or(EngineError::NoDocument)
    }
    pub fn doc_mut(&mut self) -> Result<&mut DocState> {
        self.active_mut().ok_or(EngineError::NoDocument)
    }
    pub fn set_active(&mut self, i: usize) {
        if i < self.docs.len() {
            self.active = Some(i);
        }
    }
    pub fn add_document(&mut self, d: DocState) -> usize {
        self.docs.push(d);
        self.active = Some(self.docs.len() - 1);
        self.docs.len() - 1
    }
    pub fn close_document(&mut self, i: usize) {
        if i < self.docs.len() {
            let d = self.docs.remove(i);
            if let Some(dir) = &self.recovery_dir {
                recovery::discard(dir, d.uid);
            }
            self.active = if self.docs.is_empty() { None } else { Some(i.min(self.docs.len() - 1)) };
        }
    }
    pub fn next_untitled(&mut self) -> String {
        self.untitled += 1;
        format!("Presentation{}", self.untitled)
    }

    /// Run command `id`. A panic inside it becomes [`EngineError::Internal`] and leaves the
    /// presentation as it was.
    pub fn execute(&mut self, id: &str, params: &Value) -> Result<Value> {
        self.guarded(id, |s| s.execute_unguarded(id, params))
    }

    fn execute_unguarded(&mut self, id: &str, params: &Value) -> Result<Value> {
        let spec = find_command(id).ok_or_else(|| EngineError::UnknownCommand(id.into()))?;
        if let Err(e) = (spec.enabled)(self) {
            let named = (e == cmd::NOTHING_SELECTED || e == cmd::SELECT_TWO) && (params.get("ids").is_some() || params.get("id").is_some());
            if !named {
                return Err(EngineError::Disabled(id.into(), e));
            }
        }
        let before = self.active().map(|d| (d.uid, d.doc.clone(), d.selection.clone()));
        let outer = self.depth == 0;
        self.depth += 1;
        let r = (spec.run)(self, params);
        self.depth -= 1;
        let r = r?;
        if let Some(st) = self.active_mut() {
            clamp_selection(st);
        }
        if let (Some((uid, old, sel)), Some(st)) = (before, self.active_mut())
            && st.uid == uid
            && !Arc::ptr_eq(&old, &st.doc)
            && st.interaction.is_none()
            && spec.undoable
            && outer
        {
            push_undo(st, HistoryEntry { label: spec.label.to_string(), doc: old, selection: sel });
        }
        if spec.journal && outer {
            self.journal.push((id.to_string(), params.clone()));
            if self.journal.len() > 10_000 {
                self.journal.drain(..1000);
            }
        }
        Ok(r)
    }

    pub fn commands(&self) -> Vec<CommandInfo> {
        command_specs().iter().map(|c| c.info(self)).collect()
    }

    /// Mutate the active presentation (copy-on-write) and bump the revision.
    pub fn edit<T>(&mut self, f: impl FnOnce(&mut Presentation, &mut Selection) -> Result<T>) -> Result<T> {
        let st = self.doc_mut()?;
        let mut doc = (*st.doc).clone();
        let mut sel = st.selection.clone();
        let r = f(&mut doc, &mut sel)?;
        connect::reroute(&mut doc, &sel);
        st.doc = Arc::new(doc);
        st.selection = sel;
        st.revision += 1;
        Ok(r)
    }

    /// Change only the selection (no undo step).
    pub fn select(&mut self, f: impl FnOnce(&Presentation, &mut Selection)) -> Result<()> {
        let st = self.doc_mut()?;
        let doc = st.doc.clone();
        f(&doc, &mut st.selection);
        clamp_selection(st);
        st.revision += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
