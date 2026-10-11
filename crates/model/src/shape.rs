//! Shape content and bounded shape-tree traversal.

use super::chart::Chart;
use super::style::{Effects, Fill, Line, ShapeStyle};
use super::table::Table;
use super::text::TextBody;
use super::{MediaId, ShapeId};
use super::{style, text};
use deckcraft_color::Rgba;
use deckcraft_geom::Xfrm;
use serde::{Deserialize, Serialize};

/// Placeholder types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhType {
    Title,
    CtrTitle,
    SubTitle,
    #[default]
    Body,
    Obj,
    Chart,
    Table,
    ClipArt,
    Diagram,
    Media,
    Picture,
    Date,
    Footer,
    SlideNum,
    Header,
    SlideImage,
}

impl PhType {
    pub fn xml(self) -> &'static str {
        match self {
            PhType::Title => "title",
            PhType::CtrTitle => "ctrTitle",
            PhType::SubTitle => "subTitle",
            PhType::Body => "body",
            PhType::Obj => "obj",
            PhType::Chart => "chart",
            PhType::Table => "tbl",
            PhType::ClipArt => "clipArt",
            PhType::Diagram => "dgm",
            PhType::Media => "media",
            PhType::Picture => "pic",
            PhType::Date => "dt",
            PhType::Footer => "ftr",
            PhType::SlideNum => "sldNum",
            PhType::Header => "hdr",
            PhType::SlideImage => "sldImg",
        }
    }
    pub fn from_xml(s: &str) -> Self {
        match s {
            "title" => PhType::Title,
            "ctrTitle" => PhType::CtrTitle,
            "subTitle" => PhType::SubTitle,
            "obj" => PhType::Obj,
            "chart" => PhType::Chart,
            "tbl" => PhType::Table,
            "clipArt" => PhType::ClipArt,
            "dgm" => PhType::Diagram,
            "media" => PhType::Media,
            "pic" => PhType::Picture,
            "dt" => PhType::Date,
            "ftr" => PhType::Footer,
            "sldNum" => PhType::SlideNum,
            "hdr" => PhType::Header,
            "sldImg" => PhType::SlideImage,
            _ => PhType::Body,
        }
    }
    pub fn is_title(self) -> bool {
        matches!(self, PhType::Title | PhType::CtrTitle)
    }
    /// The prompt shown in an empty placeholder in Normal view.
    pub fn prompt(self) -> &'static str {
        match self {
            PhType::Title | PhType::CtrTitle => "Click to add title",
            PhType::SubTitle => "Click to add subtitle",
            PhType::Body | PhType::Obj => "Click to add text",
            PhType::Chart => "Click icon to add chart",
            PhType::Table => "Click icon to add table",
            PhType::Diagram => "Click icon to add SmartArt graphic",
            PhType::Media => "Click icon to add media",
            PhType::Picture | PhType::ClipArt => "Click icon to add picture",
            PhType::Date => "Date",
            PhType::Footer => "Footer",
            PhType::SlideNum => "‹#›",
            PhType::Header => "Header",
            PhType::SlideImage => "",
        }
    }
    /// Footer-row placeholders are not shown on slides unless the deck turns them on.
    pub fn is_footer_kind(self) -> bool {
        matches!(self, PhType::Date | PhType::Footer | PhType::SlideNum | PhType::Header)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Placeholder {
    pub kind: PhType,
    pub idx: u32,
    /// `vert` orientation flag.
    pub vertical: bool,
    /// `quarter`, `half`, `full` size hint.
    pub size: Option<String>,
    pub has_custom_prompt: bool,
}

/// Shape geometry: a preset name with adjust values, or a custom path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Geom {
    Preset {
        name: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        adj: Vec<f64>,
    },
    /// Custom geometry: sub-paths in their own coordinate space (w, h) scaled to the shape.
    Custom { paths: Vec<CustomPath> },
}

impl Default for Geom {
    fn default() -> Self {
        Geom::Preset { name: "rect".into(), adj: vec![] }
    }
}

impl Geom {
    pub fn preset(name: &str) -> Self {
        Geom::Preset { name: name.into(), adj: vec![] }
    }
    pub fn preset_name(&self) -> Option<&str> {
        match self {
            Geom::Preset { name, .. } => Some(name),
            Geom::Custom { .. } => None,
        }
    }
}

/// One sub-path of a custom geometry: SVG-like commands in path units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomPath {
    pub w: f64,
    pub h: f64,
    /// `M x y`, `L x y`, `C x1 y1 x2 y2 x y`, `Q x1 y1 x y`, `Z`.
    pub d: String,
    pub fill: deckcraft_geom::preset::FillMode,
    pub stroke: bool,
}

impl Default for CustomPath {
    fn default() -> Self {
        CustomPath { w: 0.0, h: 0.0, d: String::new(), fill: deckcraft_geom::preset::FillMode::Norm, stroke: true }
    }
}

/// Embedded audio or video on a slide.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MediaClip {
    pub media: MediaId,
    pub video: bool,
    /// Poster frame image.
    pub poster: Option<MediaId>,
    pub trim_start_ms: u32,
    pub trim_end_ms: u32,
    pub fade_in_ms: u32,
    pub fade_out_ms: u32,
    /// 0–1.
    pub volume: f64,
    pub autoplay: bool,
    pub loop_play: bool,
    pub rewind: bool,
    pub play_across_slides: bool,
    pub hide_while_not_playing: bool,
    pub full_screen: bool,
    /// Bookmarks (name, ms).
    pub bookmarks: Vec<(String, u32)>,
    /// Probed length in ms (0 = unknown).
    pub duration_ms: u32,
    /// Probed picture size of a video in pixels (0 = unknown).
    pub width: u32,
    pub height: u32,
}

/// One ink stroke (Draw tab): points in slide coordinates with pressure.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InkStroke {
    pub points: Vec<(f64, f64, f32)>,
    pub color: Rgba,
    pub width: f64,
    pub highlighter: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ShapeKind {
    /// An autoshape or text box.
    Shape,
    Picture {
        fill: style::PictureFill,
    },
    Group {
        children: Vec<Shape>,
        /// Child coordinate space.
        child: Xfrm,
    },
    Connector {
        start: Option<(ShapeId, u32)>,
        end: Option<(ShapeId, u32)>,
    },
    Table(Table),
    Chart(Box<Chart>),
    Media(MediaClip),
    Ink {
        strokes: Vec<InkStroke>,
    },
    /// Something we read but don't model (SmartArt without fallback, OLE…): its XML is kept and its
    /// fallback picture, if any, is shown.
    Opaque {
        xml: String,
        preview: Option<MediaId>,
        label: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Shape {
    pub id: ShapeId,
    pub name: String,
    /// `None`: placeholder inheriting its box from the layout/master.
    pub xfrm: Option<Xfrm>,
    pub kind: ShapeKind,
    pub geom: Geom,
    pub ph: Option<Placeholder>,
    /// `None`: inherited (placeholder chain or style); `Some(Fill::None)`: no fill.
    pub fill: Option<Fill>,
    pub line: Option<Line>,
    pub effects: Option<Effects>,
    pub style: Option<ShapeStyle>,
    pub text: Option<TextBody>,
    pub hidden: bool,
    pub locked: bool,
    /// Text box (`txBox="1"`): no fill, autosize, wrap off by default.
    pub text_box: bool,
    /// Alt text.
    pub descr: String,
    pub title: String,
    pub decorative: bool,
    pub click: Option<text::Hyperlink>,
    pub hover: Option<text::Hyperlink>,
    /// Unmodelled XML kept for round-trip (extension lists etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_ext: Option<String>,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            id: ShapeId(0),
            name: String::new(),
            xfrm: None,
            kind: ShapeKind::Shape,
            geom: Geom::default(),
            ph: None,
            fill: None,
            line: None,
            effects: None,
            style: None,
            text: None,
            hidden: false,
            locked: false,
            text_box: false,
            descr: String::new(),
            title: String::new(),
            decorative: false,
            click: None,
            hover: None,
            raw_ext: None,
        }
    }
}

impl Shape {
    pub fn is_placeholder(&self) -> bool {
        self.ph.is_some()
    }
    pub fn ph_type(&self) -> Option<PhType> {
        self.ph.as_ref().map(|p| p.kind)
    }
    pub fn is_group(&self) -> bool {
        matches!(self.kind, ShapeKind::Group { .. })
    }
    pub fn children(&self) -> &[Shape] {
        match &self.kind {
            ShapeKind::Group { children, .. } => children,
            _ => &[],
        }
    }
    pub fn children_mut(&mut self) -> Option<&mut Vec<Shape>> {
        match &mut self.kind {
            ShapeKind::Group { children, .. } => Some(children),
            _ => None,
        }
    }
    /// Is this a line-like shape (no area)?
    pub fn is_line(&self) -> bool {
        matches!(self.kind, ShapeKind::Connector { .. }) || self.geom.preset_name().is_some_and(deckcraft_geom::preset::is_line_like)
    }
    pub fn kind_name(&self) -> &'static str {
        match &self.kind {
            ShapeKind::Shape if self.ph.is_some() => "placeholder",
            ShapeKind::Shape if self.text_box => "textBox",
            ShapeKind::Shape => "shape",
            ShapeKind::Picture { .. } => "picture",
            ShapeKind::Group { .. } => "group",
            ShapeKind::Connector { .. } => "connector",
            ShapeKind::Table(_) => "table",
            ShapeKind::Chart(_) => "chart",
            ShapeKind::Media(m) if m.video => "video",
            ShapeKind::Media(_) => "audio",
            ShapeKind::Ink { .. } => "ink",
            ShapeKind::Opaque { .. } => "object",
        }
    }
}

/// Walk shapes depth-first (groups included).
pub fn walk<'a>(shapes: &'a [Shape], f: &mut dyn FnMut(&'a Shape, usize)) {
    fn rec<'a>(shapes: &'a [Shape], depth: usize, f: &mut dyn FnMut(&'a Shape, usize)) {
        if depth > 64 {
            return;
        }
        for s in shapes {
            f(s, depth);
            rec(s.children(), depth + 1, f);
        }
    }
    rec(shapes, 0, f);
}

pub fn find_shape(shapes: &[Shape], id: ShapeId) -> Option<&Shape> {
    fn rec(shapes: &[Shape], id: ShapeId, depth: usize) -> Option<&Shape> {
        if depth > 64 {
            return None;
        }
        for s in shapes {
            if s.id == id {
                return Some(s);
            }
            if let Some(f) = rec(s.children(), id, depth + 1) {
                return Some(f);
            }
        }
        None
    }
    rec(shapes, id, 0)
}

pub fn find_shape_mut(shapes: &mut [Shape], id: ShapeId) -> Option<&mut Shape> {
    fn rec(shapes: &mut [Shape], id: ShapeId, depth: usize) -> Option<&mut Shape> {
        if depth > 64 {
            return None;
        }
        for s in shapes.iter_mut() {
            if s.id == id {
                return Some(s);
            }
            if let ShapeKind::Group { children, .. } = &mut s.kind
                && let Some(f) = rec(children, id, depth + 1)
            {
                return Some(f);
            }
        }
        None
    }
    rec(shapes, id, 0)
}
