//! Design tab and masters: themes, colours, fonts, backgrounds, slide size, header & footer,
//! Slide Master view.

mod slide_format;
use slide_format::{background, header_footer, hide_bg_graphics, slide_size};

mod master;
use master::{close_master, delete_layout, insert_layout, insert_placeholder, master_view, rename_layout};

use std::sync::Arc;

use deckcraft_model::Presentation;
use deckcraft_model::theme::{builtin_color_schemes, builtin_font_schemes, builtin_themes};
use serde_json::{Value, json};

use super::*;
use crate::{Result, Session, Target};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("design.theme", "Themes", ["Design", "Themes"], None, "{name} (see design.themes)", has_doc, theme),
        cmd!(query "design.themes", "Theme Gallery", [], None, "{} → themes, colour schemes and font schemes", always, themes),
        cmd!("design.colors", "Colors", ["Design", "Variants"], None, "{name} | {colors: {accent1: #RRGGBB, …}}", has_doc, colors),
        cmd!("design.fonts", "Fonts", ["Design", "Variants"], None, "{name} | {major, minor}", has_doc, fonts),
        cmd!(
            "design.background",
            "Format Background",
            ["Design", "Customize"],
            None,
            "{color? | gradient? | picture?: base64 | style?: 1..12 | reset?, all?: bool (apply to all), index?}",
            has_slide,
            background
        ),
        cmd!("design.hideBackgroundGraphics", "Hide Background Graphics", ["Format Background"], None, "{hide?: bool}", has_slide, hide_bg_graphics),
        cmd!(
            "design.slideSize",
            "Slide Size",
            ["Design", "Customize"],
            None,
            "{preset?: widescreen|standard|… | w, h: pt, scale?: maximize|ensureFit|none}",
            has_doc,
            slide_size
        ),
        cmd!(
            "design.headerFooter",
            "Header and Footer…",
            ["Insert", "Text"],
            None,
            "{date?: bool, dateText?: fixed text, slideNumber?: bool, footer?: bool, footerText?, hideOnTitle?: bool, all?: bool (default true)}",
            has_doc,
            header_footer
        ),
        cmd!(noundo "view.slideMaster", "Slide Master", ["View", "Master Views"], None, "{master?: index, layout?: index}", has_doc, master_view),
        cmd!(noundo "view.closeMaster", "Close Master View", ["Slide Master", "Close"], None, "{}", has_doc, close_master),
        cmd!("master.insertLayout", "Insert Layout", ["Slide Master", "Edit Master"], None, "{name?}", has_doc, insert_layout),
        cmd!("master.renameLayout", "Rename Layout", ["Slide Master", "Edit Master"], None, "{name, master?, layout?}", has_doc, rename_layout),
        cmd!("master.deleteLayout", "Delete Layout", ["Slide Master", "Edit Master"], None, "{master?, layout?}", has_doc, delete_layout),
        cmd!(
            "master.insertPlaceholder",
            "Insert Placeholder",
            ["Slide Master", "Master Layout"],
            None,
            "{kind: content|text|picture|chart|table|media, rect?: [x,y,w,h]}",
            has_doc,
            insert_placeholder
        ),
    ]
}

fn theme(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("design.theme", "missing `name`"))?;
    let t =
        builtin_themes().into_iter().find(|t| t.name.eq_ignore_ascii_case(name)).ok_or_else(|| bad("design.theme", format!("no theme `{name}`")))?;
    s.edit(|doc, sel| {
        let mi = master_index(doc, sel);
        let m = doc.masters.get_mut(mi).ok_or_else(|| bad("design.theme", "no master"))?;
        let m = Arc::make_mut(m);
        m.theme = t.clone();
        m.name = t.name.clone();
        m.color_map = vec![];
        Ok(())
    })?;
    Ok(json!({"theme": name}))
}

fn master_index(doc: &Presentation, sel: &crate::Selection) -> usize {
    match sel.target {
        Target::Master { master } | Target::Layout { master, .. } => master,
        Target::Slides => doc.slides.get(sel.slide).and_then(|s| doc.masters.iter().position(|m| m.layout(s.layout).is_some())).unwrap_or(0),
    }
}

fn themes(_s: &mut Session, _p: &Value) -> Result<Value> {
    Ok(json!({
        "themes": builtin_themes().iter().map(|t| json!({"name": t.name, "major": t.fonts.major.latin, "minor": t.fonts.minor.latin, "colors": t.colors.colors.iter().map(|c| format!("#{}", c.hex())).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "colorSchemes": builtin_color_schemes().iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
        "fontSchemes": builtin_font_schemes().iter().map(|f| json!({"name": f.name, "major": f.major.latin, "minor": f.minor.latin})).collect::<Vec<_>>(),
    }))
}

fn colors(s: &mut Session, p: &Value) -> Result<Value> {
    let scheme = if let Some(name) = str_param(p, "name") {
        Some(
            builtin_color_schemes()
                .into_iter()
                .find(|c| c.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| bad("design.colors", format!("no colour scheme `{name}`")))?,
        )
    } else {
        None
    };
    let custom = p.get("colors").and_then(Value::as_object).cloned();
    s.edit(|doc, sel| {
        let mi = master_index(doc, sel);
        let m = Arc::make_mut(doc.masters.get_mut(mi).ok_or_else(|| bad("design.colors", "no master"))?);
        if let Some(sc) = &scheme {
            m.theme.colors = sc.clone();
        }
        if let Some(c) = &custom {
            for (k, v) in c {
                if let (Some(slot), Some(rgb)) = (deckcraft_color::SchemeSlot::from_xml(k), v.as_str().and_then(deckcraft_color::Rgba::from_hex)) {
                    m.theme.colors.set(slot, rgb);
                }
            }
            m.theme.colors.name = "Custom".into();
        }
        Ok(())
    })?;
    ok()
}

fn fonts(s: &mut Session, p: &Value) -> Result<Value> {
    let scheme = if let Some(name) = str_param(p, "name") {
        builtin_font_schemes()
            .into_iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| bad("design.fonts", format!("no font scheme `{name}`")))?
    } else {
        let major = str_param(p, "major").ok_or_else(|| bad("design.fonts", "give `name`, or `major` and `minor`"))?;
        let minor = str_param(p, "minor").unwrap_or(major);
        deckcraft_model::theme::FontScheme {
            name: "Custom".into(),
            major: deckcraft_model::theme::FontSet { latin: major.into(), ..Default::default() },
            minor: deckcraft_model::theme::FontSet { latin: minor.into(), ..Default::default() },
        }
    };
    s.edit(|doc, sel| {
        let mi = master_index(doc, sel);
        Arc::make_mut(doc.masters.get_mut(mi).ok_or_else(|| bad("design.fonts", "no master"))?).theme.fonts = scheme;
        Ok(())
    })?;
    ok()
}

/// Today's date as M/D/YYYY (no clock crate: computed from the system time).
pub fn today() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let days = (secs / 86_400) as i64;
        let (y, m, d) = civil(days);
        format!("{m}/{d}/{y}")
    }
    #[cfg(target_arch = "wasm32")]
    {
        String::new()
    }
}

/// Days since 1970-01-01 → (year, month, day) (Howard Hinnant's algorithm).
pub fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    #[test]
    fn civil_dates() {
        assert_eq!(super::civil(0), (1970, 1, 1));
        assert_eq!(super::civil(20_734), (2026, 10, 8));
    }
}
