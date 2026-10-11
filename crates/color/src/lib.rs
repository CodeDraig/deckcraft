//! DeckCraft colour: sRGB colours, theme colour slots and DrawingML-style colour transforms
//! (tint, shade, luminance modulation/offset, alpha…), so theme-relative colours resolve the way
//! presentation files expect.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod transform;
pub use transform::{ColorTransform, apply};

mod scheme;
pub use scheme::{ColorScheme, SchemeSlot, theme_variants};

use serde::{Deserialize, Serialize};

/// An sRGB colour with straight alpha, components 0–255.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for Rgba {
    fn default() -> Self {
        Rgba::BLACK
    }
}

impl Rgba {
    pub const BLACK: Rgba = Rgba::rgb(0, 0, 0);
    pub const WHITE: Rgba = Rgba::rgb(255, 255, 255);
    pub const TRANSPARENT: Rgba = Rgba { r: 0, g: 0, b: 0, a: 0 };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Rgba { r, g, b, a: 255 }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Rgba { r, g, b, a }
    }
    /// `RRGGBB` or `#RRGGBB` (also `RRGGBBAA`).
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('#');
        if !s.is_ascii() {
            return None;
        }
        let byte = |i: usize| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
        match s.len() {
            6 => Some(Rgba::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Rgba::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            3 => {
                let n = |i: usize| s.get(i..i + 1).and_then(|h| u8::from_str_radix(h, 16).ok()).map(|v| v * 17);
                Some(Rgba::rgb(n(0)?, n(1)?, n(2)?))
            }
            _ => None,
        }
    }
    /// `RRGGBB` uppercase (no alpha).
    pub fn hex(&self) -> String {
        format!("{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
    pub fn with_alpha(self, a: u8) -> Self {
        Rgba { a, ..self }
    }
    pub fn to_f32(self) -> [f32; 4] {
        [self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0, self.a as f32 / 255.0]
    }
    pub fn from_f64(r: f64, g: f64, b: f64, a: f64) -> Self {
        let c = |v: f64| if v.is_finite() { (v.clamp(0.0, 1.0) * 255.0).round() as u8 } else { 0 };
        Rgba { r: c(r), g: c(g), b: c(b), a: c(a) }
    }
    /// Relative luminance (WCAG), 0–1.
    pub fn luminance(self) -> f64 {
        let lin = |c: u8| {
            let v = c as f64 / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }
    /// WCAG contrast ratio between two colours (1–21).
    pub fn contrast(self, other: Rgba) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
    pub fn to_hsl(self) -> (f64, f64, f64) {
        let r = self.r as f64 / 255.0;
        let g = self.g as f64 / 255.0;
        let b = self.b as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if (max - min).abs() < 1e-12 {
            return (0.0, 0.0, l);
        }
        let d = max - min;
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let h = if (max - r).abs() < 1e-12 {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if (max - g).abs() < 1e-12 {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h * 60.0, s, l)
    }
    pub fn from_hsl(h: f64, s: f64, l: f64, a: u8) -> Self {
        let h = ((h % 360.0) + 360.0) % 360.0 / 360.0;
        let s = s.clamp(0.0, 1.0);
        let l = l.clamp(0.0, 1.0);
        if s == 0.0 {
            return Rgba::from_f64(l, l, l, a as f64 / 255.0);
        }
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        let f = |t: f64| {
            let t = if t < 0.0 {
                t + 1.0
            } else if t > 1.0 {
                t - 1.0
            } else {
                t
            };
            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        };
        Rgba::from_f64(f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0), a as f64 / 255.0)
    }
    /// Linear interpolation (straight alpha).
    pub fn lerp(self, o: Rgba, t: f64) -> Rgba {
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        let m = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round().clamp(0.0, 255.0) as u8;
        Rgba { r: m(self.r, o.r), g: m(self.g, o.g), b: m(self.b, o.b), a: m(self.a, o.a) }
    }
}

/// Standard colours row of the colour picker.
pub const STANDARD_COLORS: [(Rgba, &str); 10] = [
    (Rgba::rgb(0xC0, 0x00, 0x00), "Dark Red"),
    (Rgba::rgb(0xFF, 0x00, 0x00), "Red"),
    (Rgba::rgb(0xFF, 0xC0, 0x00), "Orange"),
    (Rgba::rgb(0xFF, 0xFF, 0x00), "Yellow"),
    (Rgba::rgb(0x92, 0xD0, 0x50), "Light Green"),
    (Rgba::rgb(0x00, 0xB0, 0x50), "Green"),
    (Rgba::rgb(0x00, 0xB0, 0xF0), "Light Blue"),
    (Rgba::rgb(0x00, 0x70, 0xC0), "Blue"),
    (Rgba::rgb(0x00, 0x20, 0x60), "Dark Blue"),
    (Rgba::rgb(0x70, 0x30, 0xA0), "Purple"),
];

/// A few CSS-like preset colour names (`prstClr`).
pub fn preset(name: &str) -> Option<Rgba> {
    let c = match name {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "lime" => (0, 255, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "cyan" | "aqua" => (0, 255, 255),
        "magenta" | "fuchsia" => (255, 0, 255),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "maroon" => (128, 0, 0),
        "navy" => (0, 0, 128),
        "olive" => (128, 128, 0),
        "purple" => (128, 0, 128),
        "teal" => (0, 128, 128),
        "orange" => (255, 165, 0),
        "pink" => (255, 192, 203),
        "brown" => (165, 42, 42),
        "gold" => (255, 215, 0),
        "ltGray" | "lightGray" => (211, 211, 211),
        "dkGray" | "darkGray" => (169, 169, 169),
        _ => return None,
    };
    Some(Rgba::rgb(c.0, c.1, c.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(Rgba::from_hex("#156082"), Some(Rgba::rgb(0x15, 0x60, 0x82)));
        assert_eq!(Rgba::rgb(0x15, 0x60, 0x82).hex(), "156082");
        assert_eq!(Rgba::from_hex("fff"), Some(Rgba::WHITE));
        assert_eq!(Rgba::from_hex("zz0000"), None);
        assert_eq!(Rgba::from_hex("ééé"), None);
        assert_eq!(Rgba::from_hex(""), None);
    }

    #[test]
    fn hsl_roundtrip() {
        for c in [Rgba::rgb(10, 200, 30), Rgba::rgb(255, 0, 0), Rgba::rgb(128, 128, 128), Rgba::rgb(21, 96, 130)] {
            let (h, s, l) = c.to_hsl();
            let d = Rgba::from_hsl(h, s, l, 255);
            assert!(
                (d.r as i32 - c.r as i32).abs() <= 1 && (d.g as i32 - c.g as i32).abs() <= 1 && (d.b as i32 - c.b as i32).abs() <= 1,
                "{c:?} {d:?}"
            );
        }
    }

    #[test]
    fn transforms() {
        let c = Rgba::rgb(0x40, 0x80, 0xC0);
        assert_eq!(apply(c, &[ColorTransform::Alpha(50000)]).a, 128);
        let lighter = apply(c, &[ColorTransform::LumMod(20000), ColorTransform::LumOff(80000)]);
        assert!(lighter.luminance() > c.luminance());
        let darker = apply(c, &[ColorTransform::LumMod(50000)]);
        assert!(darker.luminance() < c.luminance());
        assert_eq!(apply(Rgba::WHITE, &[ColorTransform::Shade(0)]), Rgba::BLACK);
        assert_eq!(apply(Rgba::BLACK, &[ColorTransform::Tint(0)]), Rgba::WHITE);
        assert_eq!(apply(c, &[ColorTransform::Inv]), Rgba::rgb(0xBF, 0x7F, 0x3F));
        // hostile values never panic
        let _ = apply(c, &[ColorTransform::LumMod(i32::MAX), ColorTransform::HueOff(i32::MIN), ColorTransform::AlphaMod(i32::MIN)]);
    }

    #[test]
    fn contrast_ratio() {
        assert!((Rgba::BLACK.contrast(Rgba::WHITE) - 21.0).abs() < 0.01);
        assert!((Rgba::WHITE.contrast(Rgba::WHITE) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn scheme_slots() {
        let mut s = ColorScheme { name: "t".into(), colors: [Rgba::BLACK; 12] };
        s.set(SchemeSlot::Accent3, Rgba::WHITE);
        assert_eq!(s.get(SchemeSlot::Accent3), Rgba::WHITE);
        assert_eq!(s.get(SchemeSlot::Tx1), Rgba::BLACK);
        for slot in SchemeSlot::THEME {
            assert_eq!(SchemeSlot::from_xml(slot.xml_name()), Some(slot));
        }
    }
}
