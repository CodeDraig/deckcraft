//! PDF export.
//!
//! Each slide is drawn by the DeckCraft renderer at print resolution (so the PDF looks exactly like
//! the slide, effects included) and overlaid with an invisible layer of real text from the text
//! layout engine with embedded font subsets, so the text can be searched, selected and copied.
//! Hyperlinks and shape actions become link annotations, slide titles become bookmarks.
//!
//! Layouts: one slide per page, notes pages (slide above its speaker notes) and handouts (1, 2, 3,
//! 4, 6 or 9 slides per page).

mod content;
use content::Exporter;

use std::collections::HashMap;

use deckcraft_model::Presentation;
use krilla::destination::XyzDestination;
use krilla::geom::{Point, Size};
use krilla::metadata::Metadata;
use krilla::outline::{Outline, OutlineNode};
use krilla::page::PageSettings;
use kurbo::Rect;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("nothing to export (no slides in range)")]
    NoPages,
    #[error("PDF writer: {0}")]
    Write(String),
}

pub type Result<T> = std::result::Result<T, PdfError>;

/// What each page shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum PageLayout {
    /// One slide per page at the slide size.
    #[default]
    Slides,
    /// Portrait page: the slide above its notes.
    Notes,
    /// Portrait page with `per_page` slides (1, 2, 3, 4, 6 or 9).
    Handouts { per_page: u8 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PdfOptions {
    pub layout: PageLayout,
    /// Raster resolution of slide artwork (dots per inch).
    pub dpi: f64,
    /// Slide indices to export (0-based); `None` = all.
    pub slides: Option<Vec<usize>>,
    pub include_hidden: bool,
    /// Invisible real-text layer (search, select, copy).
    pub text_layer: bool,
    /// Thin frame around each slide (notes/handouts).
    pub frame_slides: bool,
    /// Document title (defaults to the presentation title).
    pub title: Option<String>,
    /// Handout/notes page size in points (default US Letter portrait).
    pub paper: (f64, f64),
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions {
            layout: PageLayout::Slides,
            dpi: 200.0,
            slides: None,
            include_hidden: false,
            text_layer: true,
            frame_slides: true,
            title: None,
            paper: (612.0, 792.0),
        }
    }
}

/// Export `pres` as PDF bytes.
pub fn export(pres: &Presentation, opts: &PdfOptions) -> Result<Vec<u8>> {
    let slides: Vec<usize> = match &opts.slides {
        Some(v) => v.iter().copied().filter(|i| *i < pres.slides.len()).collect(),
        None => (0..pres.slides.len()).filter(|i| opts.include_hidden || !pres.slides[*i].hidden).collect(),
    };
    if slides.is_empty() {
        return Err(PdfError::NoPages);
    }
    let mut pdf = krilla::Document::new();
    let title = opts.title.clone().unwrap_or_else(|| pres.props.title.clone());
    let mut meta = Metadata::new().creator("DeckCraft".into()).producer("DeckCraft".into());
    if !title.trim().is_empty() {
        meta = meta.title(title);
    }
    if !pres.props.author.trim().is_empty() {
        meta = meta.authors(vec![pres.props.author.clone()]);
    }
    pdf.set_metadata(meta);

    let (sw, sh) = (pres.slide_size.width.max(1.0), pres.slide_size.height.max(1.0));
    let mut ex = Exporter { pres, opts, fonts: HashMap::new() };
    // Page plan: per page, the slides on it with their boxes (page points).
    let pages: Vec<Vec<(usize, Rect)>> = match opts.layout {
        PageLayout::Slides => slides.iter().map(|&i| vec![(i, Rect::new(0.0, 0.0, sw, sh))]).collect(),
        PageLayout::Notes => {
            let (pw, _) = opts.paper;
            let w = pw - 2.0 * 72.0;
            let h = w * sh / sw;
            slides.iter().map(|&i| vec![(i, Rect::new(72.0, 54.0, 72.0 + w, 54.0 + h))]).collect()
        }
        PageLayout::Handouts { per_page } => {
            let boxes = handout_boxes(per_page, opts.paper, sw / sh);
            slides.chunks(boxes.len()).map(|c| c.iter().zip(&boxes).map(|(i, b)| (*i, *b)).collect()).collect()
        }
    };
    // Bookmarks and slide → page lookup for internal links.
    let page_of: HashMap<usize, usize> = pages.iter().enumerate().flat_map(|(p, v)| v.iter().map(move |(i, _)| (*i, p))).collect();
    let mut outline = Outline::new();
    for (p, v) in pages.iter().enumerate() {
        for (i, _) in v {
            let t = pres.slides[*i].title();
            let t = t.lines().next().unwrap_or("").trim().to_string();
            let label = if t.is_empty() { format!("Slide {}", i + 1) } else { format!("{}. {t}", i + 1) };
            outline.push_child(OutlineNode::new(label, XyzDestination::new(p, Point::from_xy(0.0, 0.0))));
        }
    }
    pdf.set_outline(outline);

    for v in &pages {
        let (pw, ph) = match opts.layout {
            PageLayout::Slides => (sw, sh),
            _ => opts.paper,
        };
        let size = Size::from_wh(pw as f32, ph as f32).ok_or(PdfError::NoPages)?;
        let mut page = pdf.start_page_with(PageSettings::new(size));
        let mut annots = Vec::new();
        {
            let mut s = page.surface();
            for (i, b) in v {
                ex.slide(&mut s, *i, *b);
                annots.extend(ex.links(*i, *b, &page_of));
            }
            match opts.layout {
                PageLayout::Notes => {
                    if let Some((i, b)) = v.first() {
                        let notes = pres.slides[*i].notes_text();
                        let area = Rect::new(72.0, b.y1 + 36.0, pw - 72.0, ph - 54.0);
                        ex.paragraphs(&mut s, &notes, area, 12.0);
                    }
                    ex.footer(&mut s, pw, ph, v.first().map(|x| x.0 + 1).unwrap_or(1));
                }
                PageLayout::Handouts { per_page } => {
                    if per_page == 3 {
                        for (_, b) in v {
                            ex.note_lines(&mut s, Rect::new(b.x1 + 24.0, b.y0, pw - 54.0, b.y1));
                        }
                    }
                    ex.footer(&mut s, pw, ph, page_of.get(&v[0].0).map(|p| p + 1).unwrap_or(1));
                }
                PageLayout::Slides => {}
            }
            s.finish();
        }
        for a in annots {
            page.add_annotation(a);
        }
        page.finish();
    }
    pdf.finish().map_err(|e| PdfError::Write(format!("{e:?}")))
}

/// Slide boxes for a handout page (portrait paper, half-inch-ish margins).
pub fn handout_boxes(per_page: u8, paper: (f64, f64), aspect: f64) -> Vec<Rect> {
    let (pw, ph) = paper;
    let (mx, top, bottom) = (54.0, 54.0, 54.0);
    let (cols, rows, notes) = match per_page {
        0 | 1 => (1, 1, false),
        2 => (1, 2, false),
        3 => (1, 3, true),
        4 => (2, 2, false),
        6 => (2, 3, false),
        _ => (3, 3, false),
    };
    let gap = 18.0;
    let aw = if notes { (pw - 2.0 * mx) * 0.5 } else { pw - 2.0 * mx };
    let ah = ph - top - bottom - 18.0;
    let cw = (aw - gap * (cols as f64 - 1.0)) / cols as f64;
    let ch = (ah - gap * (rows as f64 - 1.0)) / rows as f64;
    let (w, h) = if cw / ch > aspect { (ch * aspect, ch) } else { (cw, cw / aspect) };
    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let cx = mx + c as f64 * (cw + gap) + (cw - w) / 2.0;
            let cy = top + r as f64 * (ch + gap) + (ch - h) / 2.0;
            let cx = if notes { mx } else { cx };
            out.push(Rect::new(cx, cy, cx + w, cy + h));
        }
    }
    out
}

#[cfg(test)]
mod tests;
