//! Slide show requests, settings and custom slide lists.

use super::super::*;
use crate::{Result, Session, UiRequest};
use serde_json::{Value, json};

pub(super) fn start(s: &mut Session, from: usize) -> Result<Value> {
    s.ui_requests.push(UiRequest::StartShow { from });
    Ok(json!({"from": from}))
}

pub(super) fn setup(s: &mut Session, p: &Value) -> Result<Value> {
    s.edit(|doc, _| {
        let sh = &mut doc.show;
        if let Some(t) = str_param(p, "type") {
            sh.show_type = t.to_string();
            if t == "kiosk" {
                sh.loop_until_esc = true;
            }
        }
        if let Some(v) = bool_param(p, "loop") {
            sh.loop_until_esc = v;
        }
        if let Some(v) = bool_param(p, "noNarration") {
            sh.without_narration = v;
        }
        if let Some(v) = bool_param(p, "noAnimation") {
            sh.without_animation = v;
        }
        if let Some(v) = bool_param(p, "useTimings") {
            sh.use_timings = v;
        }
        match (p.get("from").and_then(Value::as_u64), p.get("to").and_then(Value::as_u64)) {
            (Some(a), Some(b)) => sh.range = Some((a.min(u32::MAX as u64) as u32, b.min(u32::MAX as u64) as u32)),
            (None, None) if p.get("all").is_some() => sh.range = None,
            _ => {}
        }
        Ok(serde_json::to_value(&*sh).unwrap_or_default())
    })
}

pub(super) fn custom_show(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("show.customShow", "missing `name`"))?.to_string();
    let delete = bool_or(p, "delete", false);
    let idx: Vec<usize> =
        p.get("slides").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(|v| v as usize).collect()).unwrap_or_default();
    s.edit(|doc, _| {
        doc.custom_shows.retain(|c| c.name != name);
        if !delete {
            let slides = idx.iter().filter_map(|i| doc.slides.get(*i).map(|x| x.id)).collect();
            doc.custom_shows.push(deckcraft_model::CustomShow { name: name.clone(), slides });
        }
        Ok(())
    })?;
    ok()
}
