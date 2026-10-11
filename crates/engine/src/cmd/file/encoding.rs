//! Presentation format recognition, serialization and render options.

use super::super::*;
use crate::{EngineError, Result};
use deckcraft_model::{Presentation, defaults};
use serde_json::Value;

/// Recognise and read presentation bytes (native, PPTX).
pub fn open_presentation(name: &str, bytes: &[u8]) -> Result<Presentation> {
    if deckcraft_format::sniff(bytes) {
        return deckcraft_format::load(bytes).map_err(|e| EngineError::Other(e.to_string()));
    }
    if deckcraft_pptx::sniff(bytes) {
        let mut p = deckcraft_pptx::import(bytes).map_err(|e| EngineError::Other(e.to_string()))?;
        deckcraft_format::repair(&mut p);
        return Ok(p);
    }
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".txt") || lower.ends_with(".md") {
        let text = String::from_utf8_lossy(bytes);
        let mut p = defaults::blank_presentation(defaults::WIDE, Default::default(), false);
        deckcraft_format::outline_to_slides(&mut p, &text);
        if p.slides.is_empty() {
            p = defaults::new_presentation(None);
        }
        return Ok(p);
    }
    Err(EngineError::Other(format!("{name}: not a presentation DeckCraft can read")))
}

/// Encode the presentation in `format` (`deckcraft`, `pptx`, `outline`).
pub fn save_bytes(doc: &Presentation, format: &str) -> Result<Vec<u8>> {
    match format {
        "pptx" | "potx" | "ppsx" => deckcraft_pptx::export(doc).map_err(|e| EngineError::Other(e.to_string())),
        "outline" | "txt" => Ok(deckcraft_format::slides_to_outline(doc).into_bytes()),
        "pdf" => deckcraft_pdf::export(doc, &Default::default()).map_err(|e| EngineError::Other(e.to_string())),
        _ => deckcraft_format::save(doc).map_err(|e| EngineError::Other(e.to_string())),
    }
}

/// PDF options from command params: `{layout: slides|notes|handouts, perPage, dpi, slides: [i],
/// includeHidden, textLayer, frame}`.
pub fn pdf_options(p: &Value) -> deckcraft_pdf::PdfOptions {
    let mut o = deckcraft_pdf::PdfOptions { layout: deckcraft_pdf::PageLayout::Slides, ..Default::default() };
    o.layout = match str_param(p, "layout").unwrap_or("slides") {
        "notes" => deckcraft_pdf::PageLayout::Notes,
        "handouts" => deckcraft_pdf::PageLayout::Handouts { per_page: usize_param(p, "perPage").unwrap_or(6).min(9) as u8 },
        _ => deckcraft_pdf::PageLayout::Slides,
    };
    o.dpi = f64_or(p, "dpi", o.dpi).clamp(36.0, 600.0);
    o.slides = p.get("slides").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect());
    o.include_hidden = bool_or(p, "includeHidden", false);
    o.text_layer = bool_or(p, "textLayer", true);
    o.frame_slides = bool_or(p, "frame", true);
    o
}

/// PDF bytes for `doc` with [`pdf_options`] from `p`.
pub fn pdf_bytes(doc: &Presentation, p: &Value) -> Result<Vec<u8>> {
    deckcraft_pdf::export(doc, &pdf_options(p)).map_err(|e| EngineError::Other(e.to_string()))
}

pub fn format_for_path(path: &str) -> &'static str {
    let l = path.to_ascii_lowercase();
    if l.ends_with(".pptx") || l.ends_with(".potx") || l.ends_with(".ppsx") {
        "pptx"
    } else if l.ends_with(".png") {
        "png"
    } else if l.ends_with(".jpg") || l.ends_with(".jpeg") {
        "jpeg"
    } else if l.ends_with(".pdf") {
        "pdf"
    } else if l.ends_with(".txt") || l.ends_with(".md") {
        "outline"
    } else {
        "deckcraft"
    }
}

/// Render slide `index` (current when absent) to PNG bytes.
pub fn render_png(doc: &Presentation, index: usize, scale: f64, edit: bool) -> (Vec<u8>, u32, u32) {
    let img =
        deckcraft_render::render_slide(doc, index, &deckcraft_render::RenderOpts { scale: scale.clamp(0.01, 16.0), edit, ..Default::default() });
    (img.to_png(), img.width, img.height)
}
