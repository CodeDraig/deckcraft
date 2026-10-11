//! Insert tab: shapes, text boxes, pictures, tables, charts, audio, video, WordArt, fields.

mod smartart;
use smartart::smart_art;

mod text_insert;
use text_insert::{action_button, date_time, hyperlink, slide_number, symbol, text_box, word_art};

mod media_insert;
use media_insert::{audio, picture, video};

mod charts;
pub use charts::chart_type;
use charts::{chart, chart_data, chart_options, chart_type_cmd};

use deckcraft_color::SchemeSlot;
use deckcraft_geom::Xfrm;
use deckcraft_model::text::{AutoFit, TextBody};
use deckcraft_model::{Geom, Shape, ShapeId, ShapeKind, ShapeStyle, Table};
use serde_json::{Value, json};

use super::*;
#[cfg(not(target_arch = "wasm32"))]
use crate::EngineError;
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "shape.insert",
            "Shapes",
            ["Insert", "Illustrations"],
            None,
            "{preset: rect|roundRect|ellipse|triangle|rightArrow|star5|… (see shape.presets), rect: [x,y,w,h] pt, text?: string}",
            has_slide,
            insert_shape
        ),
        cmd!(query "shape.presets", "Shape Gallery", [], None, "{} → [{name, label, category}]", always, presets),
        cmd!("insert.textBox", "Text Box", ["Insert", "Text"], None, "{rect?: [x,y,w,h], text?: string}", has_slide, text_box),
        cmd!(
            "insert.picture",
            "Picture from File…",
            ["Insert", "Images", "Pictures"],
            None,
            "{path? | data: base64, name?, rect?: [x,y,w,h]}",
            has_slide,
            picture
        ),
        cmd!("insert.table", "Table…", ["Insert", "Tables"], None, "{rows, cols, rect?}", has_slide, table),
        cmd!(
            "insert.chart",
            "Chart…",
            ["Insert", "Illustrations"],
            None,
            "{type?: column|bar|line|pie|doughnut|area|scatter|stackedColumn|…, rect?, categories?: [..], series?: [{name, values}]}",
            has_slide,
            chart
        ),
        cmd!("insert.audio", "Audio from File…", ["Insert", "Media", "Audio"], None, "{path? | data: base64, name?, rect?}", has_slide, audio),
        cmd!("insert.video", "Video from File…", ["Insert", "Media", "Video"], None, "{path? | data: base64, name?, rect?}", has_slide, video),
        cmd!("insert.wordArt", "WordArt", ["Insert", "Text"], None, "{text?, style?: 0..}", has_slide, word_art),
        cmd!("insert.slideNumber", "Slide Number", ["Insert", "Text"], None, "{}", has_slide, slide_number),
        cmd!("insert.dateTime", "Date & Time", ["Insert", "Text"], None, "{format?: datetime1..}", has_slide, date_time),
        cmd!("insert.symbol", "Symbol", ["Insert", "Symbols"], None, "{text: character(s)}", has_slide, symbol),
        cmd!("insert.hyperlink", "Link", ["Insert", "Links"], Some("Cmd+K"), "{url? | slide?: index, tooltip?, ids?}", has_text_or_shapes, hyperlink),
        cmd!("chart.type", "Change Chart Type", ["Chart Design", "Type"], None, "{type, id?}", has_selection, chart_type_cmd),
        cmd!(
            "chart.data",
            "Edit Data",
            ["Chart Design", "Data"],
            None,
            "{categories?: [..], series?: [{name, values}], id?}",
            has_selection,
            chart_data
        ),
        cmd!(
            "chart.options",
            "Chart Elements",
            ["Chart Design", "Chart Layouts"],
            None,
            "{title?: string|null, legend?: b|t|l|r|null, dataLabels?: bool, gridlines?: bool, palette?, id?}",
            has_selection,
            chart_options
        ),
        cmd!(
            "insert.smartArt",
            "SmartArt",
            ["Insert", "Illustrations"],
            None,
            "{kind: list|process|cycle|hierarchy|pyramid|matrix, items: [text]}",
            has_slide,
            smart_art
        ),
        cmd!(
            "insert.actionButton",
            "Action Buttons",
            ["Insert", "Illustrations"],
            None,
            "{kind: back|forward|beginning|end|home|information|return|movie|document|sound|help|custom, rect?}",
            has_slide,
            action_button
        ),
    ]
}

/// Default size for a click-inserted shape (1 inch square, PowerPoint style).
fn default_rect(s: &Session, w: f64, h: f64) -> Xfrm {
    let size = s.active().map(|d| d.doc.slide_size).unwrap_or(deckcraft_model::defaults::WIDE);
    Xfrm::new((size.width - w) / 2.0, (size.height - h) / 2.0, w, h)
}

pub(crate) fn add_shape(s: &mut Session, mut shape: Shape, select: bool) -> Result<ShapeId> {
    s.edit(|doc, sel| {
        shape.id = ShapeId(doc.alloc_id());
        if shape.name.is_empty() {
            let base = match &shape.kind {
                ShapeKind::Picture { .. } => "Picture".to_string(),
                ShapeKind::Table(_) => "Table".to_string(),
                ShapeKind::Chart(_) => "Chart".to_string(),
                ShapeKind::Media(m) if m.video => "Video".to_string(),
                ShapeKind::Media(_) => "Audio".to_string(),
                _ if shape.text_box => "TextBox".to_string(),
                _ => shape
                    .geom
                    .preset_name()
                    .and_then(deckcraft_geom::preset::info)
                    .map(|i| i.label.split(':').next().unwrap_or(i.label).trim().to_string())
                    .unwrap_or_else(|| "Shape".into()),
            };
            let n = super::current_shapes(doc, sel).map(|v| v.len()).unwrap_or(0) + 1;
            shape.name = format!("{base} {n}");
        }
        let id = shape.id;
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad("insert", "no slide"))?;
        list.push(shape);
        if select {
            sel.shapes = vec![id];
            sel.text = None;
        }
        Ok(id)
    })
}

fn insert_shape(s: &mut Session, p: &Value) -> Result<Value> {
    let preset = str_param(p, "preset").unwrap_or("rect");
    let info = deckcraft_geom::preset::info(preset).ok_or_else(|| bad("shape.insert", format!("unknown preset `{preset}`")))?;
    let line_like = deckcraft_geom::preset::is_line_like(preset);
    let rect = rect_param(p, "rect").unwrap_or_else(|| default_rect(s, 72.0, if line_like { 0.0 } else { 72.0 }));
    let adj: Vec<f64> = p.get("adj").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    let mut shape = Shape {
        xfrm: Some(rect),
        geom: Geom::Preset { name: info.name.into(), adj },
        style: Some(if line_like { ShapeStyle::line(SchemeSlot::Accent1) } else { ShapeStyle::accent(SchemeSlot::Accent1) }),
        ..Default::default()
    };
    if let Some(fl) = p.get("flip").and_then(Value::as_str)
        && let Some(x) = shape.xfrm.as_mut()
    {
        x.flip_h = fl.contains('h');
        x.flip_v = fl.contains('v');
    }
    if !line_like {
        shape.text = Some(TextBody { paragraphs: vec![], ..Default::default() });
        if let Some(t) = str_param(p, "text") {
            shape.text = Some(TextBody::from_text(t));
        }
        if let Some(tb) = shape.text.as_mut() {
            for para in &mut tb.paragraphs {
                para.props.align = Some(deckcraft_model::text::Align::Center);
            }
        }
    } else {
        shape.kind = if preset.contains("Connector") { ShapeKind::Connector { start: None, end: None } } else { ShapeKind::Shape };
        if preset == "straightConnector1" {
            shape.line = Some(deckcraft_model::Line {
                tail: Some(deckcraft_model::style::LineEnd { kind: "triangle".into(), w: "med".into(), len: "med".into() }),
                ..Default::default()
            });
        }
    }
    if let (Some(look), false) = (&s.default_look, line_like) {
        super::format::apply_painted(&mut shape, look);
    }
    let id = add_shape(s, shape, true)?;
    Ok(json!({"id": id}))
}

fn presets(_s: &mut Session, _p: &Value) -> Result<Value> {
    Ok(Value::Array(
        deckcraft_geom::preset::CATALOG.iter().map(|i| json!({"name": i.name, "label": i.label, "category": i.category.label()})).collect(),
    ))
}

pub(crate) fn new_text_box(rect: Xfrm, text: &str) -> Shape {
    let mut body = TextBody::from_text(text);
    body.body.wrap = Some(true);
    body.body.autofit = Some(AutoFit::Shape);
    Shape { xfrm: Some(rect), text_box: true, fill: None, text: Some(body), geom: Geom::preset("rect"), ..Default::default() }
}

/// Read media bytes from `data` (base64) or `path`.
pub(crate) fn media_bytes(p: &Value, cmd: &str) -> Result<(String, Vec<u8>)> {
    if let Some(d) = str_param(p, "data") {
        let bytes = base64_decode(d).ok_or_else(|| bad(cmd, "invalid base64 `data`"))?;
        return Ok((str_param(p, "name").unwrap_or("media").to_string(), bytes));
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = str_param(p, "path") {
        let bytes = std::fs::read(path).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "media".into());
        return Ok((name, bytes));
    }
    Err(bad(cmd, "missing `path` or `data`"))
}

pub fn content_type(name: &str, bytes: &[u8]) -> &'static str {
    let l = name.to_ascii_lowercase();
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        "audio/wav"
    } else if bytes.starts_with(b"BM") {
        "image/bmp"
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        "image/tiff"
    } else if bytes.starts_with(b"fLaC") {
        "audio/flac"
    } else if bytes.starts_with(b"OggS") {
        if l.ends_with(".ogv") { "video/ogg" } else { "audio/ogg" }
    } else if bytes.len() > 1 && bytes[0] == 0xFF && bytes[1] & 0xF6 == 0xF0 {
        // ADTS (layer bits 00): AAC.
        "audio/aac"
    } else if bytes.starts_with(b"ID3") || (bytes.len() > 1 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0 && !l.ends_with(".aac")) {
        "audio/mpeg"
    } else if bytes.starts_with(b"FORM") {
        "audio/aiff"
    } else if bytes.get(4..8) == Some(b"ftyp") {
        if l.ends_with(".m4a") || l.ends_with(".m4b") || bytes.get(8..11) == Some(b"M4A") {
            "audio/mp4"
        } else if l.ends_with(".mov") || bytes.get(8..12) == Some(b"qt  ") {
            "video/quicktime"
        } else {
            "video/mp4"
        }
    } else if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        if l.ends_with(".mka") || l.ends_with(".weba") {
            "audio/webm"
        } else if l.ends_with(".mkv") {
            "video/x-matroska"
        } else {
            "video/webm"
        }
    } else if bytes.starts_with(&[0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11]) {
        if l.ends_with(".wmv") || l.ends_with(".asf") { "video/x-ms-wmv" } else { "audio/x-ms-wma" }
    } else if bytes.starts_with(b"caff") {
        "audio/x-caf"
    } else if l.ends_with(".svg") || bytes.windows(4).take(512).any(|w| w == b"<svg") {
        "image/svg+xml"
    } else if l.ends_with(".aac") {
        "audio/aac"
    } else {
        "application/octet-stream"
    }
}

fn table(s: &mut Session, p: &Value) -> Result<Value> {
    let rows = usize_param(p, "rows").unwrap_or(2).clamp(1, 75);
    let cols = usize_param(p, "cols").unwrap_or(2).clamp(1, 75);
    let size = s.doc()?.doc.slide_size;
    let w = size.width * 0.8;
    let rh = 40.0;
    let rect = rect_param(p, "rect").unwrap_or(Xfrm::new(size.width * 0.1, (size.height - rh * rows as f64).max(0.0) / 2.0, w, rh * rows as f64));
    let mut t = Table::new(rows, cols, rect.w, rect.h / rows as f64);
    if let Some(data) = p.get("data").and_then(Value::as_array) {
        for (r, row) in data.iter().enumerate() {
            for (c, v) in row.as_array().map(|a| a.as_slice()).unwrap_or(&[]).iter().enumerate() {
                if let (Some(cell), Some(text)) = (t.cell_mut(r, c), v.as_str()) {
                    cell.text = TextBody::from_text(text);
                }
            }
        }
    }
    let id = add_shape(s, Shape { xfrm: Some(rect), kind: ShapeKind::Table(t), ..Default::default() }, true)?;
    Ok(json!({"id": id}))
}
