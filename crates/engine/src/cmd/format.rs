//! Home ▸ Font and Paragraph, Format menu: character, paragraph and text box formatting, and
//! the Format Painter.

mod character;
pub use character::SIZES;
use character::{
    bold, caps, case, clear, color, font, grow, highlight, italic, shrink, size, spacing, strike, subscript, superscript, text_outline, text_shadow,
    underline,
};

mod paragraph;
use paragraph::{align, align_to, bullets, level, line_spacing, numbering, paragraph};

mod body;
use body::{anchor, autofit, columns, direction, margins, wrap};

mod painter;
pub use painter::Painted;
pub(crate) use painter::apply_painted;
use painter::{painter_apply, painter_pick};

use deckcraft_model::edit as ed;
use deckcraft_model::resolve;
use deckcraft_model::text::{Align, Bullet, ParaProps, RunProps, Strike};
use serde_json::{Value, json};

use super::*;
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("format.bold", "Bold", ["Home", "Font"], Some("Cmd+B"), "{on?: bool}", has_text_or_shapes, bold),
        cmd!("format.italic", "Italic", ["Home", "Font"], Some("Cmd+I"), "{on?: bool}", has_text_or_shapes, italic),
        cmd!(
            "format.underline",
            "Underline",
            ["Home", "Font"],
            Some("Cmd+U"),
            "{on?: bool, style?: sng|dbl|heavy|dotted|dash|wavy}",
            has_text_or_shapes,
            underline
        ),
        cmd!(
            "format.strikethrough",
            "Strikethrough",
            ["Home", "Font"],
            Some("Cmd+Shift+X"),
            "{on?: bool, double?: bool}",
            has_text_or_shapes,
            strike
        ),
        cmd!("format.superscript", "Superscript", ["Home", "Font"], Some("Cmd+Shift+="), "{on?: bool}", has_text_or_shapes, superscript),
        cmd!("format.subscript", "Subscript", ["Home", "Font"], Some("Cmd+="), "{on?: bool}", has_text_or_shapes, subscript),
        cmd!("format.font", "Font", ["Home", "Font"], None, "{family}", has_text_or_shapes, font),
        cmd!("format.size", "Font Size", ["Home", "Font"], None, "{size: pt}", has_text_or_shapes, size),
        cmd!("format.grow", "Increase Font Size", ["Home", "Font"], Some("Cmd+Shift+."), "{}", has_text_or_shapes, grow),
        cmd!("format.shrink", "Decrease Font Size", ["Home", "Font"], Some("Cmd+Shift+,"), "{}", has_text_or_shapes, shrink),
        cmd!(
            "format.color",
            "Font Color",
            ["Home", "Font"],
            None,
            "{color: #RRGGBB | accent1..6 | tx1… | {scheme, lumMod…}}",
            has_text_or_shapes,
            color
        ),
        cmd!("format.highlight", "Text Highlight Color", ["Home", "Font"], None, "{color? (absent = none)}", has_text_or_shapes, highlight),
        cmd!("format.case", "Change Case", ["Home", "Font"], Some("Shift+F3"), "{mode: sentence|lower|upper|title|toggle}", has_text_or_shapes, case),
        cmd!("format.caps", "Caps", [], None, "{caps: none|small|all}", has_text_or_shapes, caps),
        cmd!(
            "format.spacing",
            "Character Spacing",
            ["Home", "Font"],
            None,
            "{pt} (veryTight -3, tight -1.5, normal 0, loose 3, veryLoose 6)",
            has_text_or_shapes,
            spacing
        ),
        cmd!("format.clear", "Clear All Formatting", ["Home", "Font"], Some("Cmd+Space"), "{}", has_text_or_shapes, clear),
        cmd!("format.textShadow", "Text Shadow", ["Home", "Font"], None, "{on?: bool}", has_text_or_shapes, text_shadow),
        cmd!(
            "format.textOutline",
            "Text Outline",
            ["Shape Format", "WordArt Styles"],
            None,
            "{color?, width?: pt} (no color = none)",
            has_text_or_shapes,
            text_outline
        ),
        cmd!("format.align", "Align", ["Home", "Paragraph"], None, "{align: left|center|right|justify|distributed}", has_text_or_shapes, align),
        cmd!("format.alignLeft", "Align Left", ["Home", "Paragraph"], Some("Cmd+L"), "{}", has_text_or_shapes, |s, _| align_to(s, Align::Left)),
        cmd!("format.alignCenter", "Center", ["Home", "Paragraph"], Some("Cmd+E"), "{}", has_text_or_shapes, |s, _| align_to(s, Align::Center)),
        cmd!("format.alignRight", "Align Right", ["Home", "Paragraph"], Some("Cmd+R"), "{}", has_text_or_shapes, |s, _| align_to(s, Align::Right)),
        cmd!("format.justify", "Justify", ["Home", "Paragraph"], Some("Cmd+J"), "{}", has_text_or_shapes, |s, _| align_to(s, Align::Justify)),
        cmd!(
            "format.bullets",
            "Bullets",
            ["Home", "Paragraph"],
            None,
            "{on?: bool, char?: •|○|■|□|◆|➢|✓|–, color?, size?: % of text}",
            has_text_or_shapes,
            bullets
        ),
        cmd!(
            "format.numbering",
            "Numbering",
            ["Home", "Paragraph"],
            None,
            "{on?: bool, scheme?: arabicPeriod|arabicParenR|romanUcPeriod|romanLcPeriod|alphaUcPeriod|alphaLcParenR|alphaLcPeriod, start?: n}",
            has_text_or_shapes,
            numbering
        ),
        cmd!("format.indent", "Increase List Level", ["Home", "Paragraph"], Some("Tab"), "{}", has_text_or_shapes, |s, _| level(s, 1)),
        cmd!("format.outdent", "Decrease List Level", ["Home", "Paragraph"], Some("Shift+Tab"), "{}", has_text_or_shapes, |s, _| level(s, -1)),
        cmd!(
            "format.lineSpacing",
            "Line Spacing",
            ["Home", "Paragraph"],
            None,
            "{lines?: 1.0|1.5|2.0…, pt?: exactly}",
            has_text_or_shapes,
            line_spacing
        ),
        cmd!(
            "format.paragraph",
            "Paragraph…",
            ["Format"],
            None,
            "{align?, indentLeft?: pt, indentFirst?: pt (negative = hanging), spaceBefore?: pt, spaceAfter?: pt, lineSpacing?: lines}",
            has_text_or_shapes,
            paragraph
        ),
        cmd!("format.anchor", "Align Text", ["Home", "Paragraph"], None, "{anchor: top|middle|bottom}", has_text_or_shapes, anchor),
        cmd!(
            "format.direction",
            "Text Direction",
            ["Home", "Paragraph"],
            None,
            "{dir: horizontal|rotate90|rotate270|stacked}",
            has_text_or_shapes,
            direction
        ),
        cmd!("format.columns", "Columns", ["Home", "Paragraph"], None, "{count, spacing?: pt}", has_text_or_shapes, columns),
        cmd!("format.autofit", "Autofit", ["Format Shape", "Text Box"], None, "{mode: none|shrink|resize}", has_text_or_shapes, autofit),
        cmd!("format.wrap", "Wrap Text in Shape", ["Format Shape", "Text Box"], None, "{on: bool}", has_text_or_shapes, wrap),
        cmd!(
            "format.margins",
            "Text Box Margins",
            ["Format Shape", "Text Box"],
            None,
            "{left?, top?, right?, bottom?: pt}",
            has_text_or_shapes,
            margins
        ),
        cmd!(noundo "format.painter", "Format Painter", ["Home", "Clipboard"], Some("Cmd+Shift+C"), "{sticky?: bool}", has_text_or_shapes, painter_pick),
        cmd!("format.painterApply", "Paste Formatting", [], Some("Cmd+Shift+V"), "{ids?}", has_text_or_shapes, painter_apply),
        cmd!(query "format.state", "Formatting State", [], None, "{} → the effective formatting at the selection (font, size, bold, …)", has_doc, state),
    ]
}

fn on_param(p: &Value, current: Option<bool>) -> bool {
    bool_param(p, "on").unwrap_or(!current.unwrap_or(false))
}

/// The effective run props at the selection (first selected run).
fn current_run(s: &Session) -> RunProps {
    let Ok(st) = s.doc() else { return RunProps::default() };
    let ctx = super::text::ctx_for(&st.doc, &st.selection);
    let (shape, para, run) = match &st.selection.text {
        Some(t) => {
            let Some(body) = super::text::body_of(st, t) else { return RunProps::default() };
            let a = t.ordered().0;
            let rp = if t.is_range() { ed::props_at(body, (a.0, a.1 + 1)) } else { ed::props_at(body, a) };
            let para = body.paragraphs.get(a.0).cloned().unwrap_or_default();
            (st.shape(t.shape).cloned().unwrap_or_default(), para, rp)
        }
        None => {
            let Some(sh) = st.selected_shapes().first().map(|x| (*x).clone()) else { return RunProps::default() };
            let para = sh.text.as_ref().and_then(|t| t.paragraphs.first().cloned()).unwrap_or_default();
            let rp = para.runs.first().map(|r| r.props.clone()).unwrap_or_default();
            (sh, para, rp)
        }
    };
    match ctx {
        Some(c) => resolve::run(&c, &shape, &para, &run),
        None => run,
    }
}

fn current_para(s: &Session) -> ParaProps {
    let Ok(st) = s.doc() else { return ParaProps::default() };
    let ctx = super::text::ctx_for(&st.doc, &st.selection);
    let (shape, para) = match &st.selection.text {
        Some(t) => {
            let para = super::text::body_of(st, t).and_then(|b| b.paragraphs.get(t.caret.0).cloned()).unwrap_or_default();
            (st.shape(t.shape).cloned().unwrap_or_default(), para)
        }
        None => {
            let Some(sh) = st.selected_shapes().first().map(|x| (*x).clone()) else { return ParaProps::default() };
            let para = sh.text.as_ref().and_then(|t| t.paragraphs.first().cloned()).unwrap_or_default();
            (sh, para)
        }
    };
    match ctx {
        Some(c) => resolve::para(&c, &shape, &para),
        None => para.props,
    }
}

fn state(s: &mut Session, _p: &Value) -> Result<Value> {
    let r = current_run(s);
    let pp = current_para(s);
    let st = s.doc()?;
    let family = super::text::ctx_for(&st.doc, &st.selection).map(|c| resolve::font_family(&c, &r)).unwrap_or_default();
    Ok(json!({
        "font": family,
        "fontRef": r.font,
        "size": r.size,
        "bold": r.bold.unwrap_or(false),
        "italic": r.italic.unwrap_or(false),
        "underline": r.underline.as_deref().is_some_and(|u| u != "none"),
        "strike": r.strike.is_some_and(|x| x != Strike::None),
        "superscript": r.baseline.unwrap_or(0.0) > 0.0,
        "subscript": r.baseline.unwrap_or(0.0) < 0.0,
        "align": pp.align.map(|a| a.xml()),
        "bullets": matches!(pp.bullet, Some(Bullet::Char { .. })),
        "numbering": matches!(pp.bullet, Some(Bullet::AutoNum { .. })),
        "lineSpacing": serde_json::to_value(pp.line_spacing).unwrap_or_default(),
        "painter": s.painter.is_some(),
    }))
}
