//! Shape Format: position and size, rotation, flips, geometry, fill, outline, effects, styles,
//! names and alt text.

mod connectors;
use connectors::{connect, freeform, sites};

mod picture;
use picture::{crop, media_options, picture_adjust, picture_change, picture_reset};

mod appearance;
pub(crate) use appearance::gradient_param;
use appearance::{effects, fill, line, quick_style};

mod transform;
use transform::{adjust, change, flip, move_by, resize, rotate, set_bounds};

use deckcraft_color::{ColorTransform, SchemeSlot};
use deckcraft_model::{ColorRef, Fill, Line, ShapeStyle};
use serde_json::Value;

use super::*;
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "shape.connect",
            "Connect Shapes",
            [],
            None,
            "{from: shape id, to: shape id, fromSite?, toSite?: site index (default: the closest pair), id?: existing line to glue (else a new connector), preset?: straightConnector1|bentConnector3|curvedConnector3} → {id}",
            has_slide,
            connect
        ),
        cmd!(
            "shape.freeform",
            "Freeform",
            ["Insert", "Shapes"],
            None,
            "{points: [[x, y], …] slide pt (≥ 2), closed?: bool (filled shape), smooth?: bool (curve through the points)} → {id}",
            has_slide,
            freeform
        ),
        cmd!(query "shape.sites", "Connection Sites", [], None, "{id} → [[x, y]] connection sites in slide points", has_slide, sites),
        cmd!("shape.move", "Move", [], None, "{dx?, dy?: pt (relative) | x?, y?: pt (absolute), ids?}", has_selection, move_by),
        cmd!("shape.resize", "Size", ["Shape Format", "Size"], None, "{w?, h?: pt, lockAspect?: bool, ids?}", has_selection, resize),
        cmd!("shape.setBounds", "Position and Size", [], None, "{x, y, w, h, ids?}", has_selection, set_bounds),
        cmd!("shape.rotate", "Rotation", ["Shape Format", "Arrange", "Rotate"], None, "{deg?: absolute, by?: relative, ids?}", has_selection, rotate),
        cmd!("shape.flip", "Flip", ["Shape Format", "Arrange", "Rotate"], None, "{axis: horizontal|vertical, ids?}", has_selection, flip),
        cmd!("shape.adjust", "Adjust Shape", [], None, "{index, value (file units), ids?}", has_selection, adjust),
        cmd!("shape.change", "Change Shape", ["Shape Format", "Insert Shapes", "Edit Shape"], None, "{preset, ids?}", has_selection, change),
        cmd!(
            "shape.fill",
            "Shape Fill",
            ["Shape Format", "Shape Styles"],
            None,
            "{color? | none?: true | gradient?: {stops: [[pos, color]], angle?, kind?: linear|radial} | picture?: base64 | pattern?: {preset, fg, bg} | transparency?: 0..1 | reset?: true, ids?}",
            has_selection,
            fill
        ),
        cmd!(
            "shape.line",
            "Shape Outline",
            ["Shape Format", "Shape Styles"],
            None,
            "{color?, none?, width?: pt, dash?: solid|dot|dash|lgDash|dashDot|sysDot|sysDash, cap?, join?, head?: none|triangle|stealth|diamond|oval|arrow, tail?, reset?, ids?}",
            has_selection,
            line
        ),
        cmd!(
            "shape.effects",
            "Shape Effects",
            ["Shape Format", "Shape Styles"],
            None,
            "{shadow?: none|outer|inner|perspective|{color, blur, dist, dir}, glow?: none|{color, radius}, softEdges?: pt, reflection?: none|tight|half|full, bevel?: none|circle|…, reset?, ids?}",
            has_selection,
            effects
        ),
        cmd!(
            "shape.quickStyle",
            "Quick Styles",
            ["Home", "Drawing"],
            None,
            "{index: 0..41 (theme style row×accent), ids?}",
            has_selection,
            quick_style
        ),
        cmd!("shape.rename", "Rename", ["Selection Pane"], None, "{name, id?}", has_selection, rename),
        cmd!("shape.altText", "Alt Text", ["Shape Format", "Accessibility"], None, "{text, decorative?: bool, ids?}", has_selection, alt_text),
        cmd!("shape.visible", "Show/Hide", ["Selection Pane"], None, "{visible?: bool, ids?}", has_selection, visible),
        cmd!("shape.lock", "Lock", [], None, "{locked?: bool, ids?}", has_selection, lock),
        cmd!("shape.setDefault", "Set as Default Shape", [], None, "{}", has_selection, set_default),
        cmd!("picture.crop", "Crop", ["Picture Format", "Size"], None, "{left?, top?, right?, bottom?: fraction 0..1, ids?}", has_selection, crop),
        cmd!(
            "picture.adjust",
            "Corrections",
            ["Picture Format", "Adjust"],
            None,
            "{brightness?: -1..1, contrast?: -1..1, saturation?: 0..4, grayscale?: bool, transparency?: 0..1, reset?: bool, ids?}",
            has_selection,
            picture_adjust
        ),
        cmd!("picture.reset", "Reset Picture", ["Picture Format", "Adjust"], None, "{ids?}", has_selection, picture_reset),
        cmd!(
            "picture.change",
            "Change Picture",
            ["Picture Format", "Adjust"],
            None,
            "{path? | data: base64, name?, ids?}",
            has_selection,
            picture_change
        ),
        cmd!(
            "media.options",
            "Playback",
            ["Playback"],
            None,
            "{autoplay?, loop?, rewind?, acrossSlides?, hide?, fullScreen?, volume?: 0..1, trimStart?: ms, trimEnd?: ms, fadeIn?: ms, fadeOut?: ms, ids?}",
            has_selection,
            media_options
        ),
    ]
}

/// Theme quick styles: 7 rows (outline only, solid fill variants, intense…) × 6 accents.
pub fn quick_style_for(index: usize) -> (ShapeStyle, Option<Fill>, Option<Line>) {
    let slots = [
        SchemeSlot::Dk1,
        SchemeSlot::Accent1,
        SchemeSlot::Accent2,
        SchemeSlot::Accent3,
        SchemeSlot::Accent4,
        SchemeSlot::Accent5,
        SchemeSlot::Accent6,
    ];
    let slot = slots.get(index % 7).copied().unwrap_or(SchemeSlot::Accent1);
    let row = index / 7;
    let mut st = ShapeStyle::accent(slot);
    let c = ColorRef::scheme(slot);
    match row {
        0 => {
            // Colored outline, light fill.
            st.fill_ref = (1, ColorRef::scheme(SchemeSlot::Lt1));
            st.line_ref = (2, c);
            st.font_ref.1 = Some(ColorRef::scheme(SchemeSlot::Dk1));
            (st, None, None)
        }
        1 => (st, None, None),
        2 => {
            st.fill_ref = (1, c.clone().with(ColorTransform::LumMod(20000)).with(ColorTransform::LumOff(80000)));
            st.line_ref = (0, c.clone());
            st.font_ref.1 = Some(c.with(ColorTransform::LumMod(50000)));
            (st, None, Some(Line::none()))
        }
        3 => {
            st.fill_ref = (2, c);
            (st, None, Some(Line::none()))
        }
        4 => {
            st.fill_ref = (3, c.clone());
            st.effect_ref = (3, c);
            (st, None, Some(Line::none()))
        }
        _ => {
            st.fill_ref = (1, c.clone().with(ColorTransform::LumMod(75000)));
            st.line_ref = (1, ColorRef::scheme(SchemeSlot::Lt1));
            (st, None, None)
        }
    }
}

fn rename(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("shape.rename", "missing `name`"))?.to_string();
    edit_shapes(s, p, "shape.rename", |sh| {
        sh.name = name.clone();
        Ok(())
    })
}

fn alt_text(s: &mut Session, p: &Value) -> Result<Value> {
    let text = str_param(p, "text").unwrap_or("").to_string();
    let deco = bool_param(p, "decorative");
    edit_shapes(s, p, "shape.altText", |sh| {
        sh.descr = text.clone();
        if let Some(d) = deco {
            sh.decorative = d;
        }
        Ok(())
    })
}

fn visible(s: &mut Session, p: &Value) -> Result<Value> {
    let v = bool_param(p, "visible");
    edit_shapes(s, p, "shape.visible", |sh| {
        sh.hidden = !v.unwrap_or(sh.hidden);
        Ok(())
    })
}

fn lock(s: &mut Session, p: &Value) -> Result<Value> {
    let v = bool_param(p, "locked");
    edit_shapes(s, p, "shape.lock", |sh| {
        sh.locked = v.unwrap_or(!sh.locked);
        Ok(())
    })
}

fn set_default(s: &mut Session, _p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let Some(sh) = st.selected_shapes().first().map(|x| (*x).clone()) else { return ok() };
    // New shapes drawn in this session take this look (Set as Default Shape).
    s.default_look = Some(super::format::Painted {
        run: sh.text.as_ref().and_then(|t| t.paragraphs.first()).and_then(|p| p.runs.first()).map(|r| r.props.clone()),
        para: None,
        fill: sh.fill.clone(),
        line: sh.line.clone(),
        effects: sh.effects.clone(),
        style: sh.style.clone(),
    });
    ok()
}

/// Path data through `pts` (shape-local): straight segments, or a Catmull-Rom curve as cubics.
pub fn freeform_path(pts: &[deckcraft_geom::Point], closed: bool, smooth: bool) -> String {
    let f = |v: f64| {
        let s = format!("{:.2}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let mut d = format!("M {} {}", f(pts[0].x), f(pts[0].y));
    let n = pts.len();
    if smooth && n > 2 {
        let at = |i: isize| -> deckcraft_geom::Point {
            if closed { pts[i.rem_euclid(n as isize) as usize] } else { pts[i.clamp(0, n as isize - 1) as usize] }
        };
        let segs = if closed { n } else { n - 1 };
        for i in 0..segs as isize {
            let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            let c1 = p1 + (p2 - p0) / 6.0;
            let c2 = p2 - (p3 - p1) / 6.0;
            d += &format!(" C {} {} {} {} {} {}", f(c1.x), f(c1.y), f(c2.x), f(c2.y), f(p2.x), f(p2.y));
        }
    } else {
        for q in &pts[1..] {
            d += &format!(" L {} {}", f(q.x), f(q.y));
        }
    }
    if closed {
        d += " Z";
    }
    d
}
