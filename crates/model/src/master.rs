//! Masters and layouts: placeholder inheritance and theme colour mapping.

use super::shape::Shape;
use super::slide::Background;
use super::text;
use super::theme::Theme;
use super::{LayoutId, MasterId};
use deckcraft_color::{ColorScheme, SchemeSlot};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutType {
    Title,
    #[default]
    TitleAndContent,
    SectionHeader,
    TwoContent,
    Comparison,
    TitleOnly,
    Blank,
    ContentWithCaption,
    PictureWithCaption,
    TitleAndVerticalText,
    VerticalTitleAndText,
    Custom,
}

impl LayoutType {
    pub fn xml(self) -> &'static str {
        match self {
            LayoutType::Title => "title",
            LayoutType::TitleAndContent => "obj",
            LayoutType::SectionHeader => "secHead",
            LayoutType::TwoContent => "twoObj",
            LayoutType::Comparison => "twoTxTwoObj",
            LayoutType::TitleOnly => "titleOnly",
            LayoutType::Blank => "blank",
            LayoutType::ContentWithCaption => "objTx",
            LayoutType::PictureWithCaption => "picTx",
            LayoutType::TitleAndVerticalText => "vertTx",
            LayoutType::VerticalTitleAndText => "vertTitleAndTx",
            LayoutType::Custom => "cust",
        }
    }
    pub fn from_xml(s: &str) -> Self {
        match s {
            "title" => LayoutType::Title,
            "obj" | "tx" => LayoutType::TitleAndContent,
            "secHead" => LayoutType::SectionHeader,
            "twoObj" | "twoColTx" => LayoutType::TwoContent,
            "twoTxTwoObj" => LayoutType::Comparison,
            "titleOnly" => LayoutType::TitleOnly,
            "blank" => LayoutType::Blank,
            "objTx" => LayoutType::ContentWithCaption,
            "picTx" => LayoutType::PictureWithCaption,
            "vertTx" => LayoutType::TitleAndVerticalText,
            "vertTitleAndTx" => LayoutType::VerticalTitleAndText,
            _ => LayoutType::Custom,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Layout {
    pub id: LayoutId,
    pub name: String,
    pub kind: LayoutType,
    pub shapes: Vec<Shape>,
    pub background: Option<Background>,
    pub show_master_shapes: bool,
    pub preserve: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_ext: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Master {
    pub id: MasterId,
    pub name: String,
    pub theme: Theme,
    pub shapes: Vec<Shape>,
    pub background: Option<Background>,
    pub title_style: text::ListStyle,
    pub body_style: text::ListStyle,
    pub other_style: text::ListStyle,
    pub layouts: Vec<Layout>,
    /// `bg1`→`lt1` etc. Default mapping when empty.
    pub color_map: Vec<(String, String)>,
    pub preserve: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_ext: Option<String>,
}

impl Master {
    pub fn layout(&self, id: LayoutId) -> Option<&Layout> {
        self.layouts.iter().find(|l| l.id == id)
    }
    /// Resolve `bg1`/`tx1`/`bg2`/`tx2` through the colour map.
    pub fn map_slot(&self, slot: SchemeSlot) -> SchemeSlot {
        let key = match slot {
            SchemeSlot::Bg1 => "bg1",
            SchemeSlot::Tx1 => "tx1",
            SchemeSlot::Bg2 => "bg2",
            SchemeSlot::Tx2 => "tx2",
            other => return other,
        };
        self.color_map.iter().find(|(k, _)| k == key).and_then(|(_, v)| SchemeSlot::from_xml(v)).unwrap_or(match slot {
            SchemeSlot::Bg1 => SchemeSlot::Lt1,
            SchemeSlot::Tx1 => SchemeSlot::Dk1,
            SchemeSlot::Bg2 => SchemeSlot::Lt2,
            _ => SchemeSlot::Dk2,
        })
    }
    /// The theme's colour scheme with the colour map applied (`bg1`… resolve to mapped colours).
    pub fn scheme(&self) -> ColorScheme {
        let mut s = self.theme.colors.clone();
        if self.color_map.is_empty() {
            return s;
        }
        let base = self.theme.colors.clone();
        for (alias, slot) in [
            (SchemeSlot::Bg1, SchemeSlot::Lt1),
            (SchemeSlot::Tx1, SchemeSlot::Dk1),
            (SchemeSlot::Bg2, SchemeSlot::Lt2),
            (SchemeSlot::Tx2, SchemeSlot::Dk2),
        ] {
            s.set(slot, base.get(self.map_slot(alias)));
        }
        s
    }
}
