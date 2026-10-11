//! Text boxes, fields, hyperlinks and action buttons.

use super::super::*;
use super::{add_shape, default_rect, new_text_box};
use crate::{Result, Session};
use deckcraft_color::SchemeSlot;
use deckcraft_geom::Xfrm;
use deckcraft_model::{ColorRef, Fill, Geom, Shape, ShapeStyle};
use serde_json::{Value, json};

pub(super) fn text_box(s: &mut Session, p: &Value) -> Result<Value> {
    let text = str_param(p, "text").unwrap_or("");
    let rect = rect_param(p, "rect").unwrap_or_else(|| default_rect(s, 216.0, 36.0));
    let mut shape = new_text_box(rect, text);
    // Click-to-create (no width) text boxes grow sideways without wrapping.
    if rect.w < 1.0 {
        shape.xfrm = Some(Xfrm { w: 100.0, ..rect });
        if let Some(t) = shape.text.as_mut() {
            t.body.wrap = Some(false);
        }
    }
    let id = add_shape(s, shape, true)?;
    super::super::text::fit_text_box(s, id)?;
    if text.is_empty() {
        // Start typing immediately.
        s.select(|_, sel| {
            sel.shapes = vec![id];
            sel.text = Some(crate::TextSel { shape: id, ..Default::default() });
        })?;
    }
    Ok(json!({"id": id}))
}

pub(super) fn word_art(s: &mut Session, p: &Value) -> Result<Value> {
    let text = str_param(p, "text").unwrap_or("Your text here");
    let size = s.doc()?.doc.slide_size;
    let style = usize_param(p, "style").unwrap_or(0);
    let mut shape = new_text_box(Xfrm::new(size.width * 0.2, size.height * 0.4, size.width * 0.6, 80.0), text);
    if let Some(t) = shape.text.as_mut() {
        for para in &mut t.paragraphs {
            para.props.align = Some(deckcraft_model::text::Align::Center);
            for r in &mut para.runs {
                r.props.size = Some(54.0);
                r.props.bold = Some(style % 2 == 1);
                r.props.fill = Some(Fill::solid(ColorRef::scheme(match style % 4 {
                    0 => SchemeSlot::Tx1,
                    1 => SchemeSlot::Accent1,
                    2 => SchemeSlot::Accent2,
                    _ => SchemeSlot::Accent4,
                })));
                if style % 3 == 2 {
                    r.props.outline = Some(deckcraft_model::Line::solid(ColorRef::scheme(SchemeSlot::Bg1), 1.0));
                }
            }
        }
    }
    let id = add_shape(s, shape, true)?;
    super::super::text::fit_text_box(s, id)?;
    Ok(json!({"id": id}))
}

pub(super) fn field_run(s: &mut Session, field: &str, text: &str) -> Result<Value> {
    if s.doc()?.selection.text.is_some() {
        return super::super::text::insert_field(s, field, text);
    }
    // Not editing: turn on the footer field for the slide (Header & Footer).
    let key = if field == "slidenum" { "slideNumber" } else { "date" };
    s.execute("design.headerFooter", &json!({key: true}))
}

pub(super) fn slide_number(s: &mut Session, _p: &Value) -> Result<Value> {
    field_run(s, "slidenum", "‹#›")
}

pub(super) fn date_time(s: &mut Session, p: &Value) -> Result<Value> {
    let f = str_param(p, "format").unwrap_or("datetime1").to_string();
    field_run(s, &f, "")
}

pub(super) fn symbol(s: &mut Session, p: &Value) -> Result<Value> {
    let t = str_param(p, "text").ok_or_else(|| bad("insert.symbol", "missing `text`"))?.to_string();
    if s.doc()?.selection.text.is_none() {
        return Err(bad("insert.symbol", "place the insertion point in text first"));
    }
    super::super::text::insert_text(s, &t, None)
}

pub(super) fn hyperlink(s: &mut Session, p: &Value) -> Result<Value> {
    use deckcraft_model::text::{Action, Hyperlink};
    let action = if let Some(u) = str_param(p, "url") {
        Action::Url { url: u.to_string() }
    } else if let Some(i) = usize_param(p, "slide") {
        let id = s.doc()?.doc.slides.get(i).map(|x| x.id).ok_or_else(|| bad("insert.hyperlink", "no such slide"))?;
        Action::Slide { slide: id }
    } else {
        return Err(bad("insert.hyperlink", "missing `url` or `slide`"));
    };
    let link = Hyperlink { action, tooltip: str_param(p, "tooltip").unwrap_or("").to_string(), highlight_click: false };
    if s.doc()?.selection.text.is_some() {
        let l = link.clone();
        return super::super::text::format_run(s, &move |rp| rp.link = Some(l.clone()));
    }
    edit_shapes(s, p, "insert.hyperlink", |sh| {
        sh.click = Some(link.clone());
        Ok(())
    })
}

pub(super) fn action_button(s: &mut Session, p: &Value) -> Result<Value> {
    use deckcraft_model::text::{Action, Hyperlink};
    let kind = str_param(p, "kind").unwrap_or("custom");
    let (preset, action) = match kind {
        "back" | "previous" => ("actionButtonBackPrevious", Some(Action::PreviousSlide)),
        "forward" | "next" => ("actionButtonForwardNext", Some(Action::NextSlide)),
        "beginning" => ("actionButtonBeginning", Some(Action::FirstSlide)),
        "end" => ("actionButtonEnd", Some(Action::LastSlide)),
        "home" => ("actionButtonHome", Some(Action::FirstSlide)),
        "information" => ("actionButtonInformation", None),
        "return" => ("actionButtonReturn", Some(Action::LastViewed)),
        "movie" => ("actionButtonMovie", None),
        "document" => ("actionButtonDocument", None),
        "sound" => ("actionButtonSound", None),
        "help" => ("actionButtonHelp", None),
        _ => ("actionButtonBlank", None),
    };
    let rect = rect_param(p, "rect").unwrap_or_else(|| default_rect(s, 48.0, 48.0));
    let shape = Shape {
        xfrm: Some(rect),
        geom: Geom::preset(preset),
        style: Some(ShapeStyle::accent(SchemeSlot::Accent1)),
        click: action.map(|a| Hyperlink { action: a, tooltip: String::new(), highlight_click: true }),
        ..Default::default()
    };
    let id = add_shape(s, shape, true)?;
    Ok(json!({"id": id}))
}
