//! Document accessibility and spelling queries.

use crate::{Result, Session};
use serde_json::{Value, json};

pub(super) fn accessibility(s: &mut Session, _p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let mut out = vec![];
    for (i, sl) in st.doc.slides.iter().enumerate() {
        if sl.title().trim().is_empty() {
            out.push(json!({"slide": i, "issue": "Missing slide title"}));
        }
        deckcraft_model::walk(&sl.shapes, &mut |sh, _| {
            let needs_alt = matches!(
                sh.kind,
                deckcraft_model::ShapeKind::Picture { .. }
                    | deckcraft_model::ShapeKind::Chart(_)
                    | deckcraft_model::ShapeKind::Media(_)
                    | deckcraft_model::ShapeKind::Group { .. }
            );
            if needs_alt && sh.descr.trim().is_empty() && !sh.decorative {
                out.push(json!({"slide": i, "shape": sh.id, "name": sh.name, "issue": "Missing alternative text"}));
            }
        });
        // Low-contrast text against the slide background.
        if let Some(ctx) = deckcraft_model::resolve::Ctx::for_slide(&st.doc, sl) {
            let bg = match deckcraft_model::resolve::background(&ctx, Some(sl)) {
                (deckcraft_model::Fill::Solid { color }, ph) => Some(ctx.color(&color, ph)),
                _ => None,
            };
            if let Some(bg) = bg {
                for sh in &sl.shapes {
                    let Some(t) = &sh.text else { continue };
                    let Some(para) = t.paragraphs.iter().find(|p| !p.is_empty()) else { continue };
                    let rp = deckcraft_model::resolve::run(&ctx, sh, para, &para.runs.first().map(|r| r.props.clone()).unwrap_or_default());
                    let (fill, _) = deckcraft_model::resolve::fill(&ctx, sh);
                    if fill.as_ref().is_some_and(|f| !f.is_none()) {
                        continue;
                    }
                    let fg = deckcraft_model::resolve::text_color(&ctx, &rp);
                    if fg.contrast(bg) < 3.0 {
                        out.push(json!({"slide": i, "shape": sh.id, "name": sh.name, "issue": format!("Hard-to-read text contrast ({:.1}:1)", fg.contrast(bg))}));
                    }
                }
            }
        }
    }
    Ok(Value::Array(out))
}

pub(super) fn spelling(s: &mut Session, _p: &Value) -> Result<Value> {
    // A small built-in list of frequent misspellings (full dictionaries come later).
    const COMMON: &[&str] = &[
        "teh",
        "recieve",
        "seperate",
        "occured",
        "untill",
        "wich",
        "becuase",
        "definately",
        "thier",
        "alot",
        "accomodate",
        "acheive",
        "adress",
        "begining",
        "beleive",
        "calender",
        "commited",
        "enviroment",
        "goverment",
        "independant",
        "neccessary",
        "occassion",
        "publically",
        "reccomend",
        "tommorow",
        "wierd",
    ];
    let st = s.doc()?;
    let mut out = vec![];
    for (i, sl) in st.doc.slides.iter().enumerate() {
        deckcraft_model::walk(&sl.shapes, &mut |sh, _| {
            if let Some(t) = &sh.text {
                for w in t.text().split(|c: char| !c.is_alphanumeric() && c != '\'') {
                    if COMMON.contains(&w.to_lowercase().as_str()) {
                        out.push(json!({"slide": i, "shape": sh.id, "word": w}));
                    }
                }
            }
        });
    }
    Ok(Value::Array(out))
}
