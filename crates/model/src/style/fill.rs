//! Shape and line fill values.

use super::ColorRef;
use crate::MediaId;
use deckcraft_color::Rgba;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradientStop {
    /// 0–1.
    pub pos: f64,
    pub color: ColorRef,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum GradientShape {
    /// Angle in degrees clockwise from left→right.
    Linear {
        angle: f64,
        #[serde(default)]
        scaled: bool,
    },
    /// `circle`, `rect` or `shape` path gradient; `focus` is the fill-to rect as fractions (l, t, r, b).
    Path { path: String, focus: [f64; 4] },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gradient {
    pub stops: Vec<GradientStop>,
    pub shape: GradientShape,
    #[serde(default = "crate::yes")]
    pub rotate_with_shape: bool,
}

/// How a picture fills its box: stretched into a (fractional) inset rect, or tiled.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum PictureMode {
    Stretch {
        #[serde(default)]
        fill_rect: [f64; 4],
    },
    Tile {
        tx: f64,
        ty: f64,
        sx: f64,
        sy: f64,
        #[serde(default)]
        flip: String,
        #[serde(default)]
        align: String,
    },
}

impl Default for PictureMode {
    fn default() -> Self {
        PictureMode::Stretch { fill_rect: [0.0; 4] }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PictureFill {
    pub media: MediaId,
    /// Source crop as fractions of the image (left, top, right, bottom); negative values pad.
    pub crop: [f64; 4],
    pub mode: PictureMode,
    /// 0–1 opacity of the picture.
    pub alpha: Option<f64>,
    pub adjust: PictureAdjust,
}

/// Picture corrections and colour (Picture Format ▸ Corrections / Color / Transparency).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PictureAdjust {
    /// -1..1
    pub brightness: f64,
    /// -1..1
    pub contrast: f64,
    /// 0..4 (1 = unchanged)
    pub saturation: Option<f64>,
    pub grayscale: bool,
    /// Recolour duotone (dark, light).
    pub duotone: Option<(ColorRef, ColorRef)>,
    /// Make a colour transparent (Set Transparent Color).
    pub clear_color: Option<Rgba>,
    pub sharpen: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternFill {
    pub preset: String,
    pub fg: ColorRef,
    pub bg: ColorRef,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Fill {
    None,
    Solid {
        color: ColorRef,
    },
    Gradient(Gradient),
    Picture(PictureFill),
    Pattern(PatternFill),
    /// Use the containing group's fill.
    Group,
    /// Use the slide background (`useBgFill`).
    Background,
}

impl Fill {
    pub fn solid(c: ColorRef) -> Self {
        Fill::Solid { color: c }
    }
    pub fn is_none(&self) -> bool {
        matches!(self, Fill::None)
    }
}
