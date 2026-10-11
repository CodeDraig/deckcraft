//! Keyboard, clipboard and synthetic input translation.

use deckcraft_engine::Mods;
use serde_json::{Value, json};

/// Engine key name for an egui key.
pub fn key_name(key: egui::Key) -> &'static str {
    use egui::Key::*;
    match key {
        Enter => "Enter",
        Escape => "Escape",
        Backspace => "Backspace",
        Delete => "Delete",
        Tab => "Tab",
        ArrowLeft => "Left",
        ArrowRight => "Right",
        ArrowUp => "Up",
        ArrowDown => "Down",
        Home => "Home",
        End => "End",
        PageUp => "PageUp",
        PageDown => "PageDown",
        Space => "Space",
        F2 => "F2",
        F5 => "F5",
        F7 => "F7",
        other => other.name(),
    }
}

impl SlideApp {
    /// Inject synthetic events (one pointer event per frame).
    pub fn raw_input_hook(&mut self, raw: &mut egui::RawInput) {
        if self.synthetic.is_empty() {
            return;
        }
        let n = match self.synthetic.first() {
            Some(egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. }) => 1,
            _ => self.synthetic.iter().position(|e| matches!(e, egui::Event::Key { pressed: false, .. })).map_or(self.synthetic.len(), |i| i + 1),
        };
        if let Some(egui::Event::PointerMoved(p) | egui::Event::PointerButton { pos: p, .. }) = self.synthetic.first() {
            raw.events.push(egui::Event::PointerMoved(*p));
        }
        let n = n.min(self.synthetic.len());
        raw.events.extend(self.synthetic.drain(..n));
    }

    /// Keyboard and clipboard for the slide (when no text field has focus).
    pub(super) fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.memory(|m| m.focused().is_some()) || self.dialog.is_some() || self.palette.is_some() {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Text(t) => {
                    if self.session.active().is_some_and(|d| d.selection.text.is_some() || d.selection.shapes.len() == 1)
                        && let Err(e) = self.session.type_text(&t)
                    {
                        self.set_status(e.to_string());
                    }
                }
                egui::Event::Copy => {
                    if let Ok(v) = self.session.execute("edit.copy", &json!({}))
                        && let Some(t) = v.get("text").and_then(Value::as_str)
                    {
                        ctx.copy_text(t.to_string());
                    }
                }
                egui::Event::Cut => {
                    if let Ok(v) = self.session.execute("edit.cut", &json!({}))
                        && let Some(t) = v.get("text").and_then(Value::as_str)
                    {
                        ctx.copy_text(t.to_string());
                    }
                }
                egui::Event::Paste(text) => {
                    // An image on the clipboard (screenshots, copied pictures) becomes a picture.
                    let img = if text.is_empty() { self.services.clipboard_image.as_mut().and_then(|f| f()) } else { None };
                    let r = match img {
                        Some(png) => self
                            .session
                            .execute("insert.picture", &json!({"name": "Pasted image.png", "data": deckcraft_engine::cmd::base64_encode(&png)})),
                        None => self.session.execute("edit.paste", &json!({"text": text})),
                    };
                    if let Err(e) = r {
                        self.set_status(e.to_string());
                    }
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    let mods = Mods { shift: modifiers.shift, alt: modifiers.alt, cmd: modifiers.command || modifiers.ctrl };
                    if !self.session.tool.drawing_freeform() && menus::ui_shortcut(self, key, mods) {
                        continue;
                    }
                    let name = key_name(key);
                    match self.session.key(name, mods) {
                        Ok(_) => {}
                        Err(e) => self.set_status(e.to_string()),
                    }
                }
                _ => {}
            }
        }
        self.drain_requests();
    }
}

use super::{SlideApp, menus};
