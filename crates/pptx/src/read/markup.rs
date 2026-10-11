//! Namespace preservation and supported alternate markup selection.

use crate::xml::{El, Node};
use std::collections::HashSet;

/// Serialize `el`, declaring every prefix it uses that the source root declared.
pub fn keep_xml(el: &El, ns: &[(String, String)]) -> String {
    let mut used = HashSet::new();
    fn rec(e: &El, used: &mut HashSet<String>, depth: usize) {
        if depth > crate::xml::MAX_DEPTH {
            return;
        }
        if let Some((p, _)) = e.name.split_once(':') {
            used.insert(p.to_string());
        }
        for (k, _) in &e.attrs {
            if let Some((p, _)) = k.split_once(':')
                && p != "xmlns"
                && p != "xml"
            {
                used.insert(p.to_string());
            }
        }
        for c in e.elements() {
            rec(c, used, depth + 1);
        }
    }
    rec(el, &mut used, 0);
    let mut el = el.clone();
    for (k, v) in ns {
        let p = k.strip_prefix("xmlns:").unwrap_or("");
        if !p.is_empty() && used.contains(p) && !el.attrs.iter().any(|(a, _)| a == k) {
            el.attrs.push((k.clone(), v.clone()));
        }
    }
    el.to_xml()
}

/// Namespace prefixes whose `mc:Choice` content we understand.
pub(super) const SUPPORTED_MC: [&str; 9] = ["p14", "p15", "p159", "a14", "a16", "p1510", "a15", "c14", "p188"];

/// Replace every `mc:AlternateContent` by the children of the branch we can read best.
pub fn resolve_mc(el: &mut El, depth: usize) {
    if depth > crate::xml::MAX_DEPTH {
        return;
    }
    if el.children.iter().any(|c| matches!(c, Node::El(e) if e.is("AlternateContent"))) {
        let mut out = Vec::with_capacity(el.children.len());
        for c in std::mem::take(&mut el.children) {
            match c {
                Node::El(ac) if ac.is("AlternateContent") => {
                    let choice = ac.children_named("Choice").find(|ch| {
                        let req_ok = ch.attr("Requires").unwrap_or("").split_whitespace().all(|r| SUPPORTED_MC.contains(&r));
                        // Ink content parts are not modelled; prefer their fallback picture.
                        let modelled = !ch.elements().any(|e| e.is("contentPart"));
                        req_ok && modelled
                    });
                    let branch = choice.or_else(|| ac.child("Fallback"));
                    if let Some(b) = branch {
                        for n in &b.children {
                            if let Node::El(e) = n {
                                let mut e = e.clone();
                                // Carry namespace declarations from the branch onto its content.
                                for (k, v) in b.attrs.iter().chain(ac.attrs.iter()) {
                                    if k.starts_with("xmlns") && !e.attrs.iter().any(|(a, _)| a == k) {
                                        e.attrs.push((k.clone(), v.clone()));
                                    }
                                }
                                out.push(Node::El(e));
                            }
                        }
                    }
                }
                other => out.push(other),
            }
        }
        el.children = out;
    }
    for c in el.elements_mut() {
        resolve_mc(c, depth + 1);
    }
}
