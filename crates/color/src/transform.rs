//! Ordered DrawingML colour transformations.

use super::Rgba;
use serde::{Deserialize, Serialize};

/// One colour transform, in file units (percentages ×1000, angles ×60000), applied in order.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "op", content = "val")]
pub enum ColorTransform {
    Tint(i32),
    Shade(i32),
    LumMod(i32),
    LumOff(i32),
    SatMod(i32),
    SatOff(i32),
    HueMod(i32),
    HueOff(i32),
    Alpha(i32),
    AlphaMod(i32),
    AlphaOff(i32),
    Comp,
    Inv,
    Gray,
    RedMod(i32),
    GreenMod(i32),
    BlueMod(i32),
}

impl ColorTransform {
    pub fn xml_name(&self) -> &'static str {
        match self {
            ColorTransform::Tint(_) => "tint",
            ColorTransform::Shade(_) => "shade",
            ColorTransform::LumMod(_) => "lumMod",
            ColorTransform::LumOff(_) => "lumOff",
            ColorTransform::SatMod(_) => "satMod",
            ColorTransform::SatOff(_) => "satOff",
            ColorTransform::HueMod(_) => "hueMod",
            ColorTransform::HueOff(_) => "hueOff",
            ColorTransform::Alpha(_) => "alpha",
            ColorTransform::AlphaMod(_) => "alphaMod",
            ColorTransform::AlphaOff(_) => "alphaOff",
            ColorTransform::Comp => "comp",
            ColorTransform::Inv => "inv",
            ColorTransform::Gray => "gray",
            ColorTransform::RedMod(_) => "redMod",
            ColorTransform::GreenMod(_) => "greenMod",
            ColorTransform::BlueMod(_) => "blueMod",
        }
    }
    pub fn value(&self) -> Option<i32> {
        match *self {
            ColorTransform::Tint(v)
            | ColorTransform::Shade(v)
            | ColorTransform::LumMod(v)
            | ColorTransform::LumOff(v)
            | ColorTransform::SatMod(v)
            | ColorTransform::SatOff(v)
            | ColorTransform::HueMod(v)
            | ColorTransform::HueOff(v)
            | ColorTransform::Alpha(v)
            | ColorTransform::AlphaMod(v)
            | ColorTransform::AlphaOff(v)
            | ColorTransform::RedMod(v)
            | ColorTransform::GreenMod(v)
            | ColorTransform::BlueMod(v) => Some(v),
            _ => None,
        }
    }
    pub fn from_xml(name: &str, val: Option<i32>) -> Option<Self> {
        let v = val.unwrap_or(0);
        Some(match name {
            "tint" => ColorTransform::Tint(v),
            "shade" => ColorTransform::Shade(v),
            "lumMod" => ColorTransform::LumMod(v),
            "lumOff" => ColorTransform::LumOff(v),
            "satMod" => ColorTransform::SatMod(v),
            "satOff" => ColorTransform::SatOff(v),
            "hueMod" => ColorTransform::HueMod(v),
            "hueOff" => ColorTransform::HueOff(v),
            "alpha" => ColorTransform::Alpha(v),
            "alphaMod" => ColorTransform::AlphaMod(v),
            "alphaOff" => ColorTransform::AlphaOff(v),
            "comp" => ColorTransform::Comp,
            "inv" => ColorTransform::Inv,
            "gray" => ColorTransform::Gray,
            "redMod" => ColorTransform::RedMod(v),
            "greenMod" => ColorTransform::GreenMod(v),
            "blueMod" => ColorTransform::BlueMod(v),
            _ => return None,
        })
    }
}

/// Apply transforms to a base colour.
pub fn apply(base: Rgba, transforms: &[ColorTransform]) -> Rgba {
    let mut c = base;
    for t in transforms {
        c = apply_one(c, *t);
    }
    c
}

pub(super) fn pct(v: i32) -> f64 {
    v as f64 / 100_000.0
}

pub(super) fn apply_one(c: Rgba, t: ColorTransform) -> Rgba {
    match t {
        // tint: towards white by (1 - value); shade: towards black by (1 - value). Office applies
        // these in linear light, which keeps mid-tones from washing out.
        ColorTransform::Tint(v) => {
            let k = pct(v).clamp(0.0, 1.0);
            map_linear(c, |x| x * k + (1.0 - k))
        }
        ColorTransform::Shade(v) => {
            let k = pct(v).clamp(0.0, 1.0);
            map_linear(c, |x| x * k)
        }
        ColorTransform::LumMod(v) => {
            let (h, s, l) = c.to_hsl();
            Rgba::from_hsl(h, s, l * pct(v), c.a)
        }
        ColorTransform::LumOff(v) => {
            let (h, s, l) = c.to_hsl();
            Rgba::from_hsl(h, s, l + pct(v), c.a)
        }
        ColorTransform::SatMod(v) => {
            let (h, s, l) = c.to_hsl();
            Rgba::from_hsl(h, s * pct(v), l, c.a)
        }
        ColorTransform::SatOff(v) => {
            let (h, s, l) = c.to_hsl();
            Rgba::from_hsl(h, s + pct(v), l, c.a)
        }
        ColorTransform::HueMod(v) => {
            let (h, s, l) = c.to_hsl();
            Rgba::from_hsl(h * pct(v), s, l, c.a)
        }
        ColorTransform::HueOff(v) => {
            let (h, s, l) = c.to_hsl();
            Rgba::from_hsl(h + v as f64 / 60_000.0, s, l, c.a)
        }
        ColorTransform::Alpha(v) => c.with_alpha((pct(v).clamp(0.0, 1.0) * 255.0).round() as u8),
        ColorTransform::AlphaMod(v) => c.with_alpha((c.a as f64 * pct(v)).round().clamp(0.0, 255.0) as u8),
        ColorTransform::AlphaOff(v) => c.with_alpha((c.a as f64 + pct(v) * 255.0).round().clamp(0.0, 255.0) as u8),
        ColorTransform::Comp => {
            let (h, s, l) = c.to_hsl();
            Rgba::from_hsl(h + 180.0, s, l, c.a)
        }
        ColorTransform::Inv => Rgba { r: 255 - c.r, g: 255 - c.g, b: 255 - c.b, a: c.a },
        ColorTransform::Gray => {
            let y = (0.299 * c.r as f64 + 0.587 * c.g as f64 + 0.114 * c.b as f64).round().clamp(0.0, 255.0) as u8;
            Rgba { r: y, g: y, b: y, a: c.a }
        }
        ColorTransform::RedMod(v) => Rgba { r: (c.r as f64 * pct(v)).round().clamp(0.0, 255.0) as u8, ..c },
        ColorTransform::GreenMod(v) => Rgba { g: (c.g as f64 * pct(v)).round().clamp(0.0, 255.0) as u8, ..c },
        ColorTransform::BlueMod(v) => Rgba { b: (c.b as f64 * pct(v)).round().clamp(0.0, 255.0) as u8, ..c },
    }
}

pub(super) fn to_lin(v: u8) -> f64 {
    let v = v as f64 / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

pub(super) fn from_lin(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

pub(super) fn map_linear(c: Rgba, f: impl Fn(f64) -> f64) -> Rgba {
    Rgba::from_f64(from_lin(f(to_lin(c.r))), from_lin(f(to_lin(c.g))), from_lin(f(to_lin(c.b))), c.a as f64 / 255.0)
}
