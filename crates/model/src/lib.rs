//! The DeckCraft document model.
//!
//! A [`Presentation`] has slide masters (each with a theme and layouts) and slides. Each slide
//! uses a layout; placeholders on a slide inherit position, formatting and text styles from the
//! matching placeholder on its layout and master ([`resolve`]). Slides and masters sit behind
//! `Arc`s so an undo snapshot of the whole deck costs one clone of the slide list.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod presentation;
pub use presentation::{CustomShow, HeaderFooter, MediaItem, Presentation, Properties, Section, ShowSettings};

mod master;
pub use master::{Layout, LayoutType, Master};

mod slide;
pub use slide::{Background, Comment, Slide};

mod shape;
pub use shape::{CustomPath, Geom, InkStroke, MediaClip, PhType, Placeholder, Shape, ShapeKind, find_shape, find_shape_mut, walk};

pub mod anim;
pub mod chart;
pub mod defaults;
pub mod edit;
pub mod resolve;
pub mod style;
pub mod table;
pub mod text;
pub mod theme;

pub use deckcraft_color::{ColorScheme, ColorTransform, Rgba, SchemeSlot};
pub use deckcraft_geom::{Rect, Size, Xfrm};
use serde::{Deserialize, Serialize};

pub use anim::{AnimClass, AnimStart, Animation, Transition};
pub use chart::Chart;
pub use style::{ColorBase, ColorRef, Effects, Fill, Line, ShapeStyle};
pub use table::{Cell, Table};
pub use text::{Paragraph, Run, RunProps, TextBody};
pub use theme::Theme;

pub(crate) fn yes() -> bool {
    true
}

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u32);
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}
id_type!(SlideId);
id_type!(ShapeId);
id_type!(LayoutId);
id_type!(MasterId);
id_type!(MediaId);

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("no slide {0}")]
    NoSlide(String),
    #[error("no shape {0}")]
    NoShape(ShapeId),
    #[error("no layout {0}")]
    NoLayout(LayoutId),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, ModelError>;

#[cfg(test)]
mod tests;
