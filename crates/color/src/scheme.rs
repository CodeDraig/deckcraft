//! Theme colour slots, scheme mapping and palette variants.

use super::{ColorTransform, Rgba};
use serde::{Deserialize, Serialize};

/// The twelve theme colour slots, plus the mapped aliases (`bg1`, `tx1`…) and `phClr` (the
/// placeholder colour a style matrix entry is applied with).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SchemeSlot {
    Dk1,
    Lt1,
    Dk2,
    Lt2,
    Accent1,
    Accent2,
    Accent3,
    Accent4,
    Accent5,
    Accent6,
    Hlink,
    FolHlink,
    Bg1,
    Tx1,
    Bg2,
    Tx2,
    PhClr,
}

impl SchemeSlot {
    pub const THEME: [SchemeSlot; 12] = [
        SchemeSlot::Dk1,
        SchemeSlot::Lt1,
        SchemeSlot::Dk2,
        SchemeSlot::Lt2,
        SchemeSlot::Accent1,
        SchemeSlot::Accent2,
        SchemeSlot::Accent3,
        SchemeSlot::Accent4,
        SchemeSlot::Accent5,
        SchemeSlot::Accent6,
        SchemeSlot::Hlink,
        SchemeSlot::FolHlink,
    ];
    pub fn xml_name(self) -> &'static str {
        match self {
            SchemeSlot::Dk1 => "dk1",
            SchemeSlot::Lt1 => "lt1",
            SchemeSlot::Dk2 => "dk2",
            SchemeSlot::Lt2 => "lt2",
            SchemeSlot::Accent1 => "accent1",
            SchemeSlot::Accent2 => "accent2",
            SchemeSlot::Accent3 => "accent3",
            SchemeSlot::Accent4 => "accent4",
            SchemeSlot::Accent5 => "accent5",
            SchemeSlot::Accent6 => "accent6",
            SchemeSlot::Hlink => "hlink",
            SchemeSlot::FolHlink => "folHlink",
            SchemeSlot::Bg1 => "bg1",
            SchemeSlot::Tx1 => "tx1",
            SchemeSlot::Bg2 => "bg2",
            SchemeSlot::Tx2 => "tx2",
            SchemeSlot::PhClr => "phClr",
        }
    }
    pub fn from_xml(s: &str) -> Option<Self> {
        Some(match s {
            "dk1" => SchemeSlot::Dk1,
            "lt1" => SchemeSlot::Lt1,
            "dk2" => SchemeSlot::Dk2,
            "lt2" => SchemeSlot::Lt2,
            "accent1" => SchemeSlot::Accent1,
            "accent2" => SchemeSlot::Accent2,
            "accent3" => SchemeSlot::Accent3,
            "accent4" => SchemeSlot::Accent4,
            "accent5" => SchemeSlot::Accent5,
            "accent6" => SchemeSlot::Accent6,
            "hlink" => SchemeSlot::Hlink,
            "folHlink" => SchemeSlot::FolHlink,
            "bg1" => SchemeSlot::Bg1,
            "tx1" => SchemeSlot::Tx1,
            "bg2" => SchemeSlot::Bg2,
            "tx2" => SchemeSlot::Tx2,
            "phClr" => SchemeSlot::PhClr,
            _ => return None,
        })
    }
    pub fn label(self) -> &'static str {
        match self {
            SchemeSlot::Dk1 | SchemeSlot::Tx1 => "Text/Background - Dark 1",
            SchemeSlot::Lt1 | SchemeSlot::Bg1 => "Text/Background - Light 1",
            SchemeSlot::Dk2 | SchemeSlot::Tx2 => "Text/Background - Dark 2",
            SchemeSlot::Lt2 | SchemeSlot::Bg2 => "Text/Background - Light 2",
            SchemeSlot::Accent1 => "Accent 1",
            SchemeSlot::Accent2 => "Accent 2",
            SchemeSlot::Accent3 => "Accent 3",
            SchemeSlot::Accent4 => "Accent 4",
            SchemeSlot::Accent5 => "Accent 5",
            SchemeSlot::Accent6 => "Accent 6",
            SchemeSlot::Hlink => "Hyperlink",
            SchemeSlot::FolHlink => "Followed Hyperlink",
            SchemeSlot::PhClr => "Placeholder colour",
        }
    }
}

/// A colour scheme: the twelve theme colours.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorScheme {
    pub name: String,
    pub colors: [Rgba; 12],
}

impl ColorScheme {
    pub fn get(&self, slot: SchemeSlot) -> Rgba {
        let i = match slot {
            SchemeSlot::Dk1 | SchemeSlot::Tx1 => 0,
            SchemeSlot::Lt1 | SchemeSlot::Bg1 => 1,
            SchemeSlot::Dk2 | SchemeSlot::Tx2 => 2,
            SchemeSlot::Lt2 | SchemeSlot::Bg2 => 3,
            SchemeSlot::Accent1 => 4,
            SchemeSlot::Accent2 => 5,
            SchemeSlot::Accent3 => 6,
            SchemeSlot::Accent4 => 7,
            SchemeSlot::Accent5 => 8,
            SchemeSlot::Accent6 => 9,
            SchemeSlot::Hlink => 10,
            SchemeSlot::FolHlink => 11,
            SchemeSlot::PhClr => 4,
        };
        self.colors.get(i).copied().unwrap_or(Rgba::BLACK)
    }
    pub fn set(&mut self, slot: SchemeSlot, c: Rgba) {
        if let Some(i) = SchemeSlot::THEME.iter().position(|s| *s == slot)
            && let Some(v) = self.colors.get_mut(i)
        {
            *v = c;
        }
    }
}

/// The colour-picker grid shown under each theme colour: lighter 80/60/40%, darker 25/50%
/// (for dark colours: lighter 90/75/50/25/10%).
pub fn theme_variants(c: Rgba) -> [(Vec<ColorTransform>, &'static str); 5] {
    let (_, _, l) = c.to_hsl();
    if l < 0.2 {
        [
            (vec![ColorTransform::LumMod(10000), ColorTransform::LumOff(90000)], "Lighter 90%"),
            (vec![ColorTransform::LumMod(25000), ColorTransform::LumOff(75000)], "Lighter 75%"),
            (vec![ColorTransform::LumMod(50000), ColorTransform::LumOff(50000)], "Lighter 50%"),
            (vec![ColorTransform::LumMod(75000), ColorTransform::LumOff(25000)], "Lighter 25%"),
            (vec![ColorTransform::LumMod(90000), ColorTransform::LumOff(10000)], "Lighter 10%"),
        ]
    } else if l > 0.8 {
        [
            (vec![ColorTransform::LumMod(95000)], "Darker 5%"),
            (vec![ColorTransform::LumMod(85000)], "Darker 15%"),
            (vec![ColorTransform::LumMod(75000)], "Darker 25%"),
            (vec![ColorTransform::LumMod(65000)], "Darker 35%"),
            (vec![ColorTransform::LumMod(50000)], "Darker 50%"),
        ]
    } else {
        [
            (vec![ColorTransform::LumMod(20000), ColorTransform::LumOff(80000)], "Lighter 80%"),
            (vec![ColorTransform::LumMod(40000), ColorTransform::LumOff(60000)], "Lighter 60%"),
            (vec![ColorTransform::LumMod(60000), ColorTransform::LumOff(40000)], "Lighter 40%"),
            (vec![ColorTransform::LumMod(75000)], "Darker 25%"),
            (vec![ColorTransform::LumMod(50000)], "Darker 50%"),
        ]
    }
}
