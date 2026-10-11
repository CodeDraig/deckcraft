//! Text outlines: slide titles and indented body paragraphs.

use deckcraft_model::text::{Paragraph, TextBody};
use deckcraft_model::{LayoutType, Presentation, defaults};
use std::sync::Arc;

/// Outline text: one slide per unindented line; tab-indented lines become body bullets at their
/// level (Insert ▸ Slides from Outline, View ▸ Outline copy).
pub fn outline_to_slides(p: &mut Presentation, text: &str) -> usize {
    let Some(lay) = defaults::layout_of_kind(p, LayoutType::TitleAndContent) else { return 0 };
    let mut made = 0;
    let mut cur: Option<deckcraft_model::Slide> = None;
    let flush = |p: &mut Presentation, s: Option<deckcraft_model::Slide>, made: &mut usize| {
        if let Some(s) = s {
            p.slides.push(Arc::new(s));
            *made += 1;
        }
    };
    for raw in text.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        let tabs = raw.chars().take_while(|c| *c == '\t').count();
        let spaces = raw.chars().take_while(|c| *c == ' ').count() / 2;
        let level = tabs.max(spaces);
        let line = raw.trim().trim_start_matches(['-', '*', '•']).trim();
        if level == 0 {
            flush(p, cur.take(), &mut made);
            let mut s = defaults::new_slide(p, lay);
            if let Some(t) = s.shapes.first_mut() {
                t.text = Some(TextBody::from_text(line));
            }
            cur = Some(s);
        } else if let Some(s) = cur.as_mut()
            && let Some(body) = s.shapes.get_mut(1)
        {
            let tb = body.text.get_or_insert_with(TextBody::default);
            let mut para = Paragraph::new(line);
            para.level = (level - 1).min(8) as u8;
            tb.paragraphs.push(para);
        }
    }
    flush(p, cur, &mut made);
    made
}

/// The deck as outline text (titles unindented, body paragraphs tab-indented by level).
pub fn slides_to_outline(p: &Presentation) -> String {
    let mut out = String::new();
    for s in &p.slides {
        out.push_str(&s.title());
        out.push('\n');
        for sh in &s.shapes {
            if sh.ph_type().is_some_and(|k| k.is_title()) {
                continue;
            }
            if let Some(t) = &sh.text {
                for para in &t.paragraphs {
                    if para.is_empty() {
                        continue;
                    }
                    for _ in 0..=para.level {
                        out.push('\t');
                    }
                    out.push_str(&para.text().replace('\u{b}', " "));
                    out.push('\n');
                }
            }
        }
    }
    out
}
