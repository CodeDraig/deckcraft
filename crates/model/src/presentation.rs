//! Presentation ownership, identifiers, media and document-level settings.

use super::master::{Layout, Master};
use super::shape::walk;
use super::slide::Slide;
use super::{LayoutId, MediaId, SlideId};
use super::{defaults, text};
use deckcraft_color::Rgba;
use deckcraft_geom::Size;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Which placeholders slides show in the footer row (Header & Footer dialog).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HeaderFooter {
    pub date: bool,
    pub slide_number: bool,
    pub footer: bool,
    pub header: bool,
    pub footer_text: String,
    /// Fixed date text; empty = automatic date.
    pub date_text: String,
    /// `datetime1`…`datetime13` format for automatic dates.
    pub date_format: String,
    pub hide_on_title: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Section {
    pub name: String,
    pub slides: Vec<SlideId>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomShow {
    pub name: String,
    pub slides: Vec<SlideId>,
}

/// Slide Show ▸ Set Up Slide Show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ShowSettings {
    /// `speaker`, `browsed`, `kiosk`.
    pub show_type: String,
    pub loop_until_esc: bool,
    pub without_narration: bool,
    pub without_animation: bool,
    pub use_timings: bool,
    /// 1-based inclusive range; `None` = all.
    pub range: Option<(u32, u32)>,
    pub custom_show: Option<String>,
    pub pen_color: Rgba,
    pub laser_color: Rgba,
    pub show_media_controls: bool,
}

impl Default for ShowSettings {
    fn default() -> Self {
        ShowSettings {
            show_type: "speaker".into(),
            loop_until_esc: false,
            without_narration: false,
            without_animation: false,
            use_timings: true,
            range: None,
            custom_show: None,
            pen_color: Rgba::rgb(0xFF, 0x00, 0x00),
            laser_color: Rgba::rgb(0xFF, 0x00, 0x00),
            show_media_controls: true,
        }
    }
}

/// Document properties (File ▸ Properties).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Properties {
    pub title: String,
    pub subject: String,
    pub author: String,
    pub keywords: String,
    pub comments: String,
    pub category: String,
    pub company: String,
    pub created: String,
    pub modified: String,
    pub last_modified_by: String,
    pub revision: u32,
}

/// An embedded file (image, audio, video, font, workbook).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MediaItem {
    pub id: MediaId,
    /// Original file name, e.g. `image1.png`.
    pub name: String,
    pub content_type: String,
    /// Bytes; kept out of the JSON manifest and stored as a separate file in native files.
    #[serde(skip)]
    pub data: Arc<Vec<u8>>,
    /// External link instead of embedded bytes.
    pub link: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Presentation {
    /// Slide size in points (default 16:9, 960 × 540).
    pub slide_size: Size,
    pub notes_size: Size,
    pub slides: Vec<Arc<Slide>>,
    pub masters: Vec<Arc<Master>>,
    pub notes_master: Option<Arc<Master>>,
    pub handout_master: Option<Arc<Master>>,
    pub sections: Vec<Section>,
    pub custom_shows: Vec<CustomShow>,
    pub show: ShowSettings,
    pub header_footer: HeaderFooter,
    pub props: Properties,
    /// Presentation-wide default text style (shapes that aren't placeholders).
    pub default_text_style: text::ListStyle,
    pub media: Vec<MediaItem>,
    pub first_slide_number: u32,
    /// Next free shape/slide/layout/media id.
    pub next_id: u32,
    /// Embedded fonts (family names; bytes in `media`).
    pub embedded_fonts: Vec<(String, MediaId)>,
    /// Unmodelled presentation-level XML kept for round-trip.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_ext: Option<String>,
}

impl Default for Presentation {
    fn default() -> Self {
        defaults::new_presentation(None)
    }
}

impl Presentation {
    /// A fresh id, unique across slides, shapes, layouts, masters and media in this deck.
    pub fn alloc_id(&mut self) -> u32 {
        self.next_id = self.next_id.max(1).saturating_add(1);
        self.next_id
    }
    /// Make sure `next_id` is above every id in use (after loading a file).
    pub fn fix_next_id(&mut self) {
        let mut m = self.next_id;
        for s in &self.slides {
            m = m.max(s.id.0);
            walk(&s.shapes, &mut |sh, _| m = m.max(sh.id.0));
        }
        for ma in &self.masters {
            m = m.max(ma.id.0);
            walk(&ma.shapes, &mut |sh, _| m = m.max(sh.id.0));
            for l in &ma.layouts {
                m = m.max(l.id.0);
                walk(&l.shapes, &mut |sh, _| m = m.max(sh.id.0));
            }
        }
        for md in &self.media {
            m = m.max(md.id.0);
        }
        self.next_id = m;
    }
    pub fn slide_index(&self, id: SlideId) -> Option<usize> {
        self.slides.iter().position(|s| s.id == id)
    }
    pub fn slide(&self, id: SlideId) -> Option<&Slide> {
        self.slides.iter().find(|s| s.id == id).map(|s| s.as_ref())
    }
    pub fn slide_mut(&mut self, id: SlideId) -> Option<&mut Slide> {
        self.slides.iter_mut().find(|s| s.id == id).map(Arc::make_mut)
    }
    pub fn slide_at(&self, i: usize) -> Option<&Slide> {
        self.slides.get(i).map(|s| s.as_ref())
    }
    /// Master and layout for a layout id.
    pub fn layout(&self, id: LayoutId) -> Option<(&Master, &Layout)> {
        self.masters.iter().find_map(|m| m.layout(id).map(|l| (m.as_ref(), l)))
    }
    /// The master (and layout) a slide uses, falling back to the first.
    pub fn master_for(&self, slide: &Slide) -> Option<(&Master, Option<&Layout>)> {
        if let Some((m, l)) = self.layout(slide.layout) {
            return Some((m, Some(l)));
        }
        self.masters.first().map(|m| (m.as_ref(), m.layouts.first()))
    }
    pub fn first_master(&self) -> Option<&Master> {
        self.masters.first().map(|m| m.as_ref())
    }
    pub fn media(&self, id: MediaId) -> Option<&MediaItem> {
        self.media.iter().find(|m| m.id == id)
    }
    /// Add media bytes, reusing an identical existing item.
    pub fn add_media(&mut self, name: &str, content_type: &str, data: Vec<u8>) -> MediaId {
        if let Some(m) = self.media.iter().find(|m| m.data.len() == data.len() && *m.data == data) {
            return m.id;
        }
        let id = MediaId(self.alloc_id());
        self.media.push(MediaItem { id, name: name.into(), content_type: content_type.into(), data: Arc::new(data), link: None });
        id
    }
    /// Section index containing a slide (by position), when sections exist.
    pub fn section_of(&self, slide: SlideId) -> Option<usize> {
        self.sections.iter().position(|s| s.slides.contains(&slide))
    }
    /// 1-based slide number as displayed (respects `first_slide_number`).
    pub fn slide_number(&self, index: usize) -> u32 {
        self.first_slide_number.saturating_add(u32::try_from(index).unwrap_or(u32::MAX))
    }
    /// Basic consistency check: unique slide/shape ids, layouts exist. Returns problems found.
    pub fn validate(&self) -> Vec<String> {
        let mut out = vec![];
        let mut seen = std::collections::HashSet::new();
        for s in &self.slides {
            if !seen.insert(s.id) {
                out.push(format!("duplicate slide id {}", s.id));
            }
            if self.layout(s.layout).is_none() {
                out.push(format!("slide {} uses missing layout {}", s.id, s.layout));
            }
            let mut ids = std::collections::HashSet::new();
            walk(&s.shapes, &mut |sh, _| {
                if !ids.insert(sh.id) {
                    out.push(format!("slide {}: duplicate shape id {}", s.id, sh.id));
                }
            });
        }
        out
    }
}
