//! Shape shadow, glow and reflection values.

use super::ColorRef;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shadow {
    pub color: ColorRef,
    /// Blur radius, points.
    pub blur: f64,
    /// Distance, points.
    pub dist: f64,
    /// Direction, degrees (0 = right, 90 = down).
    pub dir: f64,
    #[serde(default)]
    pub inner: bool,
    /// Scale (1 = 100%) for perspective shadows.
    #[serde(default = "one")]
    pub sx: f64,
    #[serde(default = "one")]
    pub sy: f64,
    #[serde(default)]
    pub kx: f64,
    #[serde(default)]
    pub ky: f64,
    #[serde(default)]
    pub align: String,
    #[serde(default = "crate::yes")]
    pub rotate_with_shape: bool,
}

pub(super) fn one() -> f64 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Glow {
    pub color: ColorRef,
    pub radius: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reflection {
    pub blur: f64,
    pub start_alpha: f64,
    pub end_alpha: f64,
    pub end_pos: f64,
    pub dist: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Effects {
    pub outer_shadow: Option<Shadow>,
    pub inner_shadow: Option<Shadow>,
    pub glow: Option<Glow>,
    /// Soft edges radius in points.
    pub soft_edge: Option<f64>,
    pub reflection: Option<Reflection>,
    /// Bevel preset name for 3-D format (rendered as an approximation).
    pub bevel: Option<String>,
    /// Unmodelled 3-D XML kept for round-trip (`scene3d`, `sp3d`).
    pub raw3d: Option<String>,
}

impl Effects {
    pub fn is_empty(&self) -> bool {
        self.outer_shadow.is_none()
            && self.inner_shadow.is_none()
            && self.glow.is_none()
            && self.soft_edge.is_none()
            && self.reflection.is_none()
            && self.bevel.is_none()
    }
}
