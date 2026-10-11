//! Format Painter capture and application lifecycle.

use super::super::text::{format_para, format_run};
use super::super::*;
use crate::{Result, Session};
use deckcraft_model::edit as ed;
use deckcraft_model::text::{ParaProps, RunProps};
use deckcraft_model::{Effects, Fill, Line, Shape};
use serde_json::{Value, json};

/// Formatting picked up by the Format Painter.
#[derive(Clone, Debug, Default)]
pub struct Painted {
    pub run: Option<RunProps>,
    pub para: Option<ParaProps>,
    pub fill: Option<Fill>,
    pub line: Option<Line>,
    pub effects: Option<Effects>,
    pub style: Option<deckcraft_model::ShapeStyle>,
}

pub(super) fn painter_pick(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let mut painted = Painted::default();
    if let Some(t) = &st.selection.text {
        if let Some(b) = super::super::text::body_of(st, t) {
            let a = t.ordered().0;
            painted.run = Some(ed::props_at(b, (a.0, a.1 + 1)));
            painted.para = b.paragraphs.get(a.0).map(|x| x.props.clone());
        }
    } else if let Some(sh) = st.selected_shapes().first() {
        painted.fill = sh.fill.clone();
        painted.line = sh.line.clone();
        painted.effects = sh.effects.clone();
        painted.style = sh.style.clone();
        painted.run = sh.text.as_ref().and_then(|t| t.paragraphs.first()).and_then(|p| p.runs.first()).map(|r| r.props.clone());
    }
    s.painter = Some((painted, bool_or(p, "sticky", false)));
    Ok(json!({"armed": true}))
}

pub(crate) fn apply_painted(sh: &mut Shape, p: &Painted) {
    if p.fill.is_some() || p.style.is_some() {
        sh.fill.clone_from(&p.fill);
        sh.line.clone_from(&p.line);
        sh.effects.clone_from(&p.effects);
        sh.style.clone_from(&p.style);
    }
    if let (Some(r), Some(t)) = (&p.run, sh.text.as_mut()) {
        ed::format_all(t, &|rp| *rp = r.clone());
    }
}

pub(super) fn painter_apply(s: &mut Session, p: &Value) -> Result<Value> {
    let Some((painted, sticky)) = s.painter.clone() else { return Err(bad("format.painterApply", "pick formatting with the Format Painter first")) };
    if !sticky {
        s.painter = None;
    }
    if s.doc()?.selection.text.as_ref().is_some_and(|t| t.is_range()) && ids_param(p, "ids").is_none() {
        if let Some(r) = painted.run.clone() {
            format_run(s, &move |rp| *rp = r.clone())?;
        }
        if let Some(pp) = painted.para.clone() {
            format_para(s, &move |para| para.props = pp.clone())?;
        }
        return ok();
    }
    edit_shapes(s, p, "format.painterApply", |sh| {
        apply_painted(sh, &painted);
        Ok(())
    })
}
