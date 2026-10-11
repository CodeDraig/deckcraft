//! Content insertion forms and equation notation.

use super::{Dialog, buttons, field, num};
use crate::SlideApp;
use crate::theme::{self};
use egui::{Ui, vec2};
use serde_json::json;

pub(super) fn pretty_equation(s: &str) -> String {
    let sup = |c: char| match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        'n' => 'ⁿ',
        'i' => 'ⁱ',
        '+' => '⁺',
        '-' => '⁻',
        _ => c,
    };
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '^' => {
                while let Some(&n) = chars.peek() {
                    if n.is_ascii_alphanumeric() || n == '+' || n == '-' {
                        out.push(sup(n));
                        chars.next();
                        if !n.is_ascii_digit() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            '*' => out.push('·'),
            _ => out.push(c),
        }
    }
    out.replace("sqrt", "√").replace("<=", "≤").replace(">=", "≥").replace("!=", "≠").replace("pi", "π").replace("+-", "±")
}

pub(super) fn table_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    field(ui, d, "Number of columns:", "cols", "5");
    field(ui, d, "Number of rows:", "rows", "2");
    let (ok, cancel) = buttons(ui, "Insert");
    if ok {
        let (r, c) = (num(d, "rows", 2.0) as usize, num(d, "cols", 5.0) as usize);
        let _ = app.run("insert.table", json!({"rows": r, "cols": c}));
    }
    ok || cancel
}

pub(super) fn outline_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    ui.label("Paste or type an outline. Unindented lines become slide titles; tab-indented lines become bullets.");
    let mut v = d.get("text", "");
    ui.add(egui::TextEdit::multiline(&mut v).desired_rows(10).desired_width(380.0).code_editor());
    d.fields.insert("text".into(), v.clone());
    let (ok, cancel) = buttons(ui, "Insert");
    if ok {
        let _ = app.run("slide.fromOutline", json!({"text": v}));
    }
    ok || cancel
}

pub(super) fn symbol_dialog(app: &mut SlideApp, ui: &mut Ui, _d: &mut Dialog) -> bool {
    ui.label("Click a symbol to insert it at the insertion point.");
    let groups: [(&str, &str); 6] = [
        ("Punctuation", "–—‘’“”…•·§¶†‡‰′″‹›«»¡¿"),
        ("Currency", "$€£¥¢₹₩₽₺₿"),
        ("Math", "±×÷≠≈≤≥∞√∑∏∫∂∆∇∈∉∩∪⊂⊃∀∃¬∧∨⇒⇔°′"),
        ("Greek", "αβγδεζηθικλμνξοπρστυφχψωΓΔΘΛΞΠΣΦΨΩ"),
        ("Arrows", "←↑→↓↔↕⇐⇑⇒⇓↖↗↘↙⟵⟶"),
        ("Shapes", "■□▪▫▲△▶▷▼▽◀◁◆◇○●◎★☆✓✔✗✘♠♣♥♦"),
    ];
    for (g, chars) in groups {
        ui.label(egui::RichText::new(g).font(theme::bold(12.0)));
        ui.horizontal_wrapped(|ui| {
            for c in chars.chars() {
                if ui.add(egui::Button::new(egui::RichText::new(c.to_string()).size(16.0)).min_size(vec2(26.0, 26.0))).clicked() {
                    if app.session.active().is_some_and(|s| s.selection.text.is_none()) {
                        let _ = app.run("text.edit", json!({}));
                    }
                    let _ = app.run("insert.symbol", json!({"text": c.to_string()}));
                }
            }
        });
    }
    let (ok, cancel) = buttons(ui, "Close");
    ok || cancel
}

pub(super) fn equation_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    ui.label("Type an equation in linear form (e.g. a^2+b^2=c^2, x=(-b±√(b^2-4ac))/2a).");
    let mut v = d.get("eq", "");
    ui.add(egui::TextEdit::singleline(&mut v).desired_width(340.0).font(theme::font(15.0)));
    d.fields.insert("eq".into(), v.clone());
    let (ok, cancel) = buttons(ui, "Insert");
    if ok && !v.is_empty() {
        let pretty = pretty_equation(&v);
        let size = app.session.active().map(|s| s.doc.slide_size).unwrap_or(deckcraft_model::defaults::WIDE);
        let _ = app.run("insert.textBox", json!({"rect": [size.width / 2.0 - 150.0, size.height / 2.0 - 25.0, 300, 50], "text": pretty}));
        let _ = app.run("text.exit", json!({}));
        let _ = app.run("format.font", json!({"family": "Liberation Serif"}));
        let _ = app.run("format.italic", json!({"on": true}));
        let _ = app.run("format.size", json!({"size": 28}));
    }
    ok || cancel
}

pub(super) fn smartart_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    ui.label("Pick a graphic; type one item per line.");
    let kind = d.get("kind", "process");
    ui.horizontal_wrapped(|ui| {
        for (k, l) in [
            ("list", "Basic List"),
            ("process", "Basic Process"),
            ("cycle", "Basic Cycle"),
            ("hierarchy", "Hierarchy"),
            ("pyramid", "Pyramid"),
            ("matrix", "Matrix"),
        ] {
            if ui.selectable_label(kind == k, l).clicked() {
                d.fields.insert("kind".into(), k.into());
            }
        }
    });
    let mut v = d.get("text", "Plan\nBuild\nLaunch");
    ui.add(egui::TextEdit::multiline(&mut v).desired_rows(5).desired_width(320.0));
    d.fields.insert("text".into(), v.clone());
    let (ok, cancel) = buttons(ui, "Insert");
    if ok {
        let _ = app.run("insert.smartArt", json!({"kind": kind, "items": v.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>()}));
    }
    ok || cancel
}
