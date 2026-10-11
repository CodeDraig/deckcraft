//! Colours, fills, lines and effects.

mod effects;
pub use effects::{Effects, Glow, Reflection, Shadow};

mod line;
pub use line::{Compound, Dash, Line, LineCap, LineEnd, LineJoin};

mod fill;
pub use fill::{Fill, Gradient, GradientShape, GradientStop, PatternFill, PictureAdjust, PictureFill, PictureMode};

use deckcraft_color::{ColorScheme, ColorTransform, Rgba, SchemeSlot};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ColorBase {
    Rgb {
        rgb: Rgba,
    },
    Scheme {
        slot: SchemeSlot,
    },
    Preset {
        name: String,
    },
    /// A system colour (`windowText`…) with the value it had when saved.
    System {
        name: String,
        last: Rgba,
    },
}

/// A colour as written in a document: a base (RGB, theme slot…) plus transforms.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorRef {
    pub base: ColorBase,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mods: Vec<ColorTransform>,
}

impl ColorRef {
    pub fn rgb(c: Rgba) -> Self {
        ColorRef { base: ColorBase::Rgb { rgb: c }, mods: vec![] }
    }
    pub fn scheme(slot: SchemeSlot) -> Self {
        ColorRef { base: ColorBase::Scheme { slot }, mods: vec![] }
    }
    pub fn with(mut self, m: ColorTransform) -> Self {
        self.mods.push(m);
        self
    }
    /// Resolve against a colour scheme; `ph` substitutes `phClr` (style matrix colour).
    pub fn resolve(&self, scheme: &ColorScheme, ph: Option<Rgba>) -> Rgba {
        let base = match &self.base {
            ColorBase::Rgb { rgb } => *rgb,
            ColorBase::Scheme { slot: SchemeSlot::PhClr } => ph.unwrap_or_else(|| scheme.get(SchemeSlot::Accent1)),
            ColorBase::Scheme { slot } => scheme.get(*slot),
            ColorBase::Preset { name } => deckcraft_color::preset(name).unwrap_or(Rgba::BLACK),
            ColorBase::System { last, .. } => *last,
        };
        deckcraft_color::apply(base, &self.mods)
    }
    /// Alpha (0–1) from an `alpha` transform, if any.
    pub fn alpha(&self) -> f64 {
        self.mods.iter().fold(1.0, |a, m| match m {
            ColorTransform::Alpha(v) => (*v as f64 / 100_000.0).clamp(0.0, 1.0),
            _ => a,
        })
    }
}

/// A shape's theme style references (`p:style`): which fill/line/effect of the theme's format scheme
/// to use, with which colour, and the theme font and colour for its text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeStyle {
    pub line_ref: (u32, ColorRef),
    pub fill_ref: (u32, ColorRef),
    pub effect_ref: (u32, ColorRef),
    /// `major`, `minor` or `none`, with the text colour.
    pub font_ref: (String, Option<ColorRef>),
}

impl ShapeStyle {
    /// The default for newly drawn shapes: accent 1 fill, darker accent 1 outline, light text.
    pub fn accent(slot: SchemeSlot) -> Self {
        ShapeStyle {
            line_ref: (2, ColorRef::scheme(slot).with(ColorTransform::Shade(50000))),
            fill_ref: (1, ColorRef::scheme(slot)),
            effect_ref: (0, ColorRef::scheme(slot)),
            font_ref: ("minor".into(), Some(ColorRef::scheme(SchemeSlot::Lt1))),
        }
    }
    /// The default for lines/connectors.
    pub fn line(slot: SchemeSlot) -> Self {
        ShapeStyle {
            line_ref: (1, ColorRef::scheme(slot)),
            fill_ref: (0, ColorRef::scheme(slot)),
            effect_ref: (0, ColorRef::scheme(slot)),
            font_ref: ("minor".into(), Some(ColorRef::scheme(SchemeSlot::Tx1))),
        }
    }
}
