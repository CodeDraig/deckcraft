//! Slides, their background, notes and review comments.

use super::anim::{Animation, Transition};
use super::shape::{PhType, Shape, find_shape, find_shape_mut};
use super::style::{ColorRef, Fill};
use super::text::TextBody;
use super::{LayoutId, ShapeId, SlideId};
use serde::{Deserialize, Serialize};

/// Background of a slide, layout or master.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Background {
    Fill {
        fill: Fill,
    },
    /// Theme background style reference (1001–1003) with a colour.
    Ref {
        idx: u32,
        color: ColorRef,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Comment {
    pub author: String,
    pub initials: String,
    pub text: String,
    /// ISO 8601.
    pub date: String,
    pub x: f64,
    pub y: f64,
    pub resolved: bool,
    pub replies: Vec<Comment>,
    /// The shape the comment is attached to.
    pub shape: Option<ShapeId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Slide {
    pub id: SlideId,
    pub layout: LayoutId,
    pub name: String,
    pub shapes: Vec<Shape>,
    pub background: Option<Background>,
    pub hidden: bool,
    pub show_master_shapes: bool,
    pub transition: Option<Transition>,
    pub animations: Vec<Animation>,
    /// Speaker notes.
    pub notes: TextBody,
    pub comments: Vec<Comment>,
    /// Unmodelled XML kept for round-trip (custom data, tags).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_ext: Option<String>,
}

impl Default for Slide {
    fn default() -> Self {
        Slide {
            id: SlideId(0),
            layout: LayoutId(0),
            name: String::new(),
            shapes: vec![],
            background: None,
            hidden: false,
            show_master_shapes: true,
            transition: None,
            animations: vec![],
            notes: TextBody::default(),
            comments: vec![],
            raw_ext: None,
        }
    }
}

impl Slide {
    pub fn shape(&self, id: ShapeId) -> Option<&Shape> {
        find_shape(&self.shapes, id)
    }
    pub fn shape_mut(&mut self, id: ShapeId) -> Option<&mut Shape> {
        find_shape_mut(&mut self.shapes, id)
    }
    /// The slide's title text (first title placeholder), for thumbnails, outline and navigation.
    pub fn title(&self) -> String {
        self.shapes
            .iter()
            .find(|s| s.ph_type().is_some_and(PhType::is_title))
            .and_then(|s| s.text.as_ref())
            .map(|t| t.text().replace('\u{b}', " "))
            .unwrap_or_default()
    }
    pub fn notes_text(&self) -> String {
        self.notes.text()
    }
}
