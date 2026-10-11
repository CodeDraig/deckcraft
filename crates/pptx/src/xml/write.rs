//! Escaping XML text and building serialized XML fragments.

use super::xml_char;
use std::fmt::Write as _;

/// Escape text content (and drop characters XML 1.0 forbids).
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            c if xml_char(c) => o.push(c),
            _ => {}
        }
    }
    o
}

/// Escape an attribute value.
pub fn esc_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\n' => o.push_str("&#10;"),
            '\r' => o.push_str("&#13;"),
            '\t' => o.push_str("&#9;"),
            c if xml_char(c) => o.push(c),
            _ => {}
        }
    }
    o
}

/// A string builder for XML parts.
#[derive(Default)]
pub struct W {
    pub s: String,
}

/// Attribute list builder.
#[derive(Default)]
pub struct A {
    pub v: Vec<(&'static str, String)>,
}

impl A {
    pub fn new() -> Self {
        A { v: vec![] }
    }
    pub fn a(mut self, k: &'static str, v: impl ToString) -> Self {
        self.v.push((k, v.to_string()));
        self
    }
    pub fn o<T: ToString>(mut self, k: &'static str, v: Option<T>) -> Self {
        if let Some(v) = v {
            self.v.push((k, v.to_string()));
        }
        self
    }
    /// Boolean attribute written as `1`/`0` when `Some`.
    pub fn b(mut self, k: &'static str, v: Option<bool>) -> Self {
        if let Some(v) = v {
            self.v.push((k, if v { "1" } else { "0" }.to_string()));
        }
        self
    }
    /// Boolean attribute written only when true.
    pub fn t(mut self, k: &'static str, v: bool) -> Self {
        if v {
            self.v.push((k, "1".to_string()));
        }
        self
    }
}

impl W {
    pub fn new() -> Self {
        W { s: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n") }
    }
    pub fn frag() -> Self {
        W { s: String::new() }
    }
    fn attrs(&mut self, a: &A) {
        for (k, v) in &a.v {
            let _ = write!(self.s, " {}=\"{}\"", k, esc_attr(v));
        }
    }
    pub fn open(&mut self, name: &str, a: A) {
        self.s.push('<');
        self.s.push_str(name);
        self.attrs(&a);
        self.s.push('>');
    }
    pub fn open0(&mut self, name: &str) {
        self.s.push('<');
        self.s.push_str(name);
        self.s.push('>');
    }
    pub fn close(&mut self, name: &str) {
        self.s.push_str("</");
        self.s.push_str(name);
        self.s.push('>');
    }
    pub fn empty(&mut self, name: &str, a: A) {
        self.s.push('<');
        self.s.push_str(name);
        self.attrs(&a);
        self.s.push_str("/>");
    }
    pub fn empty0(&mut self, name: &str) {
        self.s.push('<');
        self.s.push_str(name);
        self.s.push_str("/>");
    }
    /// `<name val="v"/>`
    pub fn val(&mut self, name: &str, v: impl ToString) {
        self.empty(name, A::new().a("val", v));
    }
    pub fn text(&mut self, t: &str) {
        self.s.push_str(&esc(t));
    }
    /// `<name>text</name>`
    pub fn elt(&mut self, name: &str, t: &str) {
        self.open0(name);
        self.text(t);
        self.close(name);
    }
    pub fn raw(&mut self, xml: &str) {
        self.s.push_str(xml);
    }
    pub fn finish(self) -> Vec<u8> {
        self.s.into_bytes()
    }
}
