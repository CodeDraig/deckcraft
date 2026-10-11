//! Line strokes, dash patterns and endpoints.

use super::ColorRef;
use super::fill::Fill;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineCap {
    #[default]
    Flat,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineJoin {
    #[default]
    Round,
    Bevel,
    Miter,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Compound {
    #[default]
    Single,
    Double,
    ThickThin,
    ThinThick,
    Triple,
}

impl Compound {
    pub fn xml(self) -> &'static str {
        match self {
            Compound::Single => "sng",
            Compound::Double => "dbl",
            Compound::ThickThin => "thickThin",
            Compound::ThinThick => "thinThick",
            Compound::Triple => "tri",
        }
    }
    pub fn from_xml(s: &str) -> Self {
        match s {
            "dbl" => Compound::Double,
            "thickThin" => Compound::ThickThin,
            "thinThick" => Compound::ThinThick,
            "tri" => Compound::Triple,
            _ => Compound::Single,
        }
    }
}

/// Dash style: a preset name (`solid`, `dot`, `dash`, `lgDash`, `dashDot`, `lgDashDot`,
/// `lgDashDotDot`, `sysDash`, `sysDot`, `sysDashDot`, `sysDashDotDot`) or custom (dash, space) pairs
/// in multiples of the line width.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Dash {
    Preset { name: String },
    Custom { pattern: Vec<(f64, f64)> },
}

impl Dash {
    pub const PRESETS: [&'static str; 11] =
        ["solid", "sysDot", "sysDash", "dash", "dashDot", "lgDash", "lgDashDot", "lgDashDotDot", "dot", "sysDashDot", "sysDashDotDot"];
    /// Dash/space lengths in multiples of the line width.
    pub fn pattern(&self) -> Vec<f64> {
        match self {
            Dash::Custom { pattern } => pattern.iter().flat_map(|(d, s)| [*d, *s]).collect(),
            Dash::Preset { name } => match name.as_str() {
                "dot" => vec![1.0, 3.0],
                "dash" => vec![4.0, 3.0],
                "lgDash" => vec![8.0, 3.0],
                "dashDot" => vec![4.0, 3.0, 1.0, 3.0],
                "lgDashDot" => vec![8.0, 3.0, 1.0, 3.0],
                "lgDashDotDot" => vec![8.0, 3.0, 1.0, 3.0, 1.0, 3.0],
                "sysDash" => vec![3.0, 1.0],
                "sysDot" => vec![1.0, 1.0],
                "sysDashDot" => vec![3.0, 1.0, 1.0, 1.0],
                "sysDashDotDot" => vec![3.0, 1.0, 1.0, 1.0, 1.0, 1.0],
                _ => vec![],
            },
        }
    }
}

/// Arrowhead at a line end.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineEnd {
    /// `none`, `triangle`, `stealth`, `diamond`, `oval`, `arrow`.
    pub kind: String,
    /// `sm`, `med`, `lg`.
    #[serde(default = "med")]
    pub w: String,
    #[serde(default = "med")]
    pub len: String,
}

pub(super) fn med() -> String {
    "med".into()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Line {
    pub fill: Option<Fill>,
    /// Width in points.
    pub width: Option<f64>,
    pub dash: Option<Dash>,
    pub cap: Option<LineCap>,
    pub join: Option<LineJoin>,
    pub compound: Option<Compound>,
    pub head: Option<LineEnd>,
    pub tail: Option<LineEnd>,
}

impl Line {
    pub fn solid(c: ColorRef, width: f64) -> Self {
        Line { fill: Some(Fill::solid(c)), width: Some(width), ..Default::default() }
    }
    pub fn none() -> Self {
        Line { fill: Some(Fill::None), ..Default::default() }
    }
    /// Fill unset fields from `parent`.
    pub fn inherit(&mut self, parent: &Line) {
        if self.fill.is_none() {
            self.fill.clone_from(&parent.fill);
        }
        self.width = self.width.or(parent.width);
        if self.dash.is_none() {
            self.dash.clone_from(&parent.dash);
        }
        self.cap = self.cap.or(parent.cap);
        self.join = self.join.or(parent.join);
        self.compound = self.compound.or(parent.compound);
        if self.head.is_none() {
            self.head.clone_from(&parent.head);
        }
        if self.tail.is_none() {
            self.tail.clone_from(&parent.tail);
        }
    }
}
