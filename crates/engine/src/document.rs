//! Editable document state: selection, snapshot history and dirty tracking.

use deckcraft_model::{Presentation, Shape, ShapeId, Slide, SlideId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Text being edited in place: a shape's body (or a table cell, or the slide's notes).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSel {
    pub shape: ShapeId,
    /// Selection anchor and caret: (paragraph, char).
    pub anchor: (usize, usize),
    pub caret: (usize, usize),
    /// Editing a table cell (row, col).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<(usize, usize)>,
    /// Editing the slide's speaker notes (shape is ignored).
    #[serde(default)]
    pub notes: bool,
}

impl TextSel {
    pub fn is_range(&self) -> bool {
        self.anchor != self.caret
    }
    pub fn ordered(&self) -> ((usize, usize), (usize, usize)) {
        if self.anchor <= self.caret { (self.anchor, self.caret) } else { (self.caret, self.anchor) }
    }
}

/// Which shape tree is being edited: slides (normal), or a master/layout (Slide Master view).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Target {
    #[default]
    Slides,
    Master {
        master: usize,
    },
    Layout {
        master: usize,
        layout: usize,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    /// Current slide index.
    pub slide: usize,
    /// Slides selected in the thumbnail pane / sorter (includes the current one when non-empty).
    pub slides: Vec<SlideId>,
    /// Selected shapes on the current slide (top-level or inside groups).
    pub shapes: Vec<ShapeId>,
    pub text: Option<TextSel>,
    /// Selected table cells: (table, (r0, c0), (r1, c1)).
    pub cells: Option<(ShapeId, (usize, usize), (usize, usize))>,
    pub target: Target,
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub label: String,
    pub doc: Arc<Presentation>,
    pub selection: Selection,
}

#[derive(Clone, Debug, Default)]
pub struct History {
    pub undo: Vec<HistoryEntry>,
    pub redo: Vec<HistoryEntry>,
    pub limit: usize,
}

/// A live pointer interaction: the document before it began (one undo step at the end).
#[derive(Clone, Debug)]
pub struct Interaction {
    pub label: String,
    pub doc: Arc<Presentation>,
    pub selection: Selection,
}

#[derive(Clone, Debug)]
pub struct DocState {
    pub doc: Arc<Presentation>,
    pub selection: Selection,
    pub history: History,
    pub path: Option<String>,
    pub revision: u64,
    pub saved_doc: Arc<Presentation>,
    pub interaction: Option<Interaction>,
    pub uid: u64,
    pub title: String,
}

pub(super) static NEXT_UID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl DocState {
    pub fn new(doc: Presentation, path: Option<String>, title: String) -> Self {
        let doc = Arc::new(doc);
        DocState {
            saved_doc: doc.clone(),
            doc,
            selection: Selection::default(),
            history: History { limit: 500, ..Default::default() },
            path,
            revision: 1,
            interaction: None,
            uid: NEXT_UID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            title,
        }
    }
    pub fn is_dirty(&self) -> bool {
        !Arc::ptr_eq(&self.doc, &self.saved_doc)
    }
    pub fn title(&self) -> String {
        self.path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_stem())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.title.clone())
    }
    pub fn current_slide(&self) -> Option<&Slide> {
        self.doc.slides.get(self.selection.slide).map(|s| s.as_ref())
    }
    /// Shapes of the edited tree (current slide, or the master/layout in Slide Master view).
    pub fn shapes(&self) -> &[Shape] {
        match self.selection.target {
            Target::Slides => self.current_slide().map(|s| s.shapes.as_slice()).unwrap_or(&[]),
            Target::Master { master } => self.doc.masters.get(master).map(|m| m.shapes.as_slice()).unwrap_or(&[]),
            Target::Layout { master, layout } => {
                self.doc.masters.get(master).and_then(|m| m.layouts.get(layout)).map(|l| l.shapes.as_slice()).unwrap_or(&[])
            }
        }
    }
    pub fn shape(&self, id: ShapeId) -> Option<&Shape> {
        deckcraft_model::find_shape(self.shapes(), id)
    }
    pub fn selected_shapes(&self) -> Vec<&Shape> {
        self.selection.shapes.iter().filter_map(|id| self.shape(*id)).collect()
    }
}

/// Mutable access to the edited shape tree of a presentation.
pub fn shapes_mut<'a>(doc: &'a mut Presentation, sel: &Selection) -> Option<&'a mut Vec<Shape>> {
    match sel.target {
        Target::Slides => doc.slides.get_mut(sel.slide).map(|s| &mut Arc::make_mut(s).shapes),
        Target::Master { master } => doc.masters.get_mut(master).map(|m| &mut Arc::make_mut(m).shapes),
        Target::Layout { master, layout } => {
            doc.masters.get_mut(master).and_then(|m| Arc::make_mut(m).layouts.get_mut(layout)).map(|l| &mut l.shapes)
        }
    }
}

/// Keep the selection pointing at things that exist.
pub(super) fn clamp_selection(st: &mut DocState) {
    let n = st.doc.slides.len();
    if n == 0 {
        st.selection.slide = 0;
    } else if st.selection.slide >= n {
        st.selection.slide = n - 1;
    }
    let ids: Vec<ShapeId> = st.selection.shapes.iter().copied().filter(|id| st.shape(*id).is_some()).collect();
    st.selection.shapes = ids;
    if let Some(t) = &st.selection.text
        && !t.notes
        && st.shape(t.shape).is_none()
    {
        st.selection.text = None;
    }
    let slides: Vec<SlideId> = st.selection.slides.iter().copied().filter(|id| st.doc.slide(*id).is_some()).collect();
    st.selection.slides = slides;
    match st.selection.target {
        Target::Master { master } if master >= st.doc.masters.len() => st.selection.target = Target::Slides,
        Target::Layout { master, layout } if st.doc.masters.get(master).is_none_or(|m| layout >= m.layouts.len()) => {
            st.selection.target = Target::Slides
        }
        _ => {}
    }
}

pub(crate) fn push_undo(st: &mut DocState, e: HistoryEntry) {
    st.history.undo.push(e);
    st.history.redo.clear();
    if st.history.undo.len() > st.history.limit.max(1) {
        st.history.undo.remove(0);
    }
}
