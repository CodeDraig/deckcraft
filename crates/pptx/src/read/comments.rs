//! Modern comment parts. Errors propagate so a failed import cannot drop a thread silently.

use std::collections::{HashMap, HashSet};

use deckcraft_model::Comment;

use super::Part;
use super::shapes::IdCtx;
use crate::PptxError;
use crate::comments::{self, CT_AUTHORS, CT_COMMENTS, NS};
use crate::opc::Package;
use crate::xml::{Doc, El};

pub(super) type Authors = HashMap<String, (String, String)>;

fn part(pkg: &Package, path: &str, content_type: &str, root: &str) -> Result<Doc, PptxError> {
    if pkg.content_type(path).as_deref() != Some(content_type) {
        return Err(PptxError::Unsupported(format!("{path}: only modern PowerPoint comments are supported")));
    }
    let doc = pkg.xml(path).ok_or_else(|| PptxError::Xml(format!("missing or unreadable comment part {path}")))?;
    let key = if doc.root.prefix().is_empty() { "xmlns".into() } else { format!("xmlns:{}", doc.root.prefix()) };
    if !doc.root.is(root) || !doc.root.attrs.iter().any(|(k, v)| k == &key && v == NS) {
        return Err(PptxError::Unsupported(format!("{path}: expected a modern {root} element")));
    }
    Ok(doc)
}

pub(super) fn authors(pkg: &Package, pres: &Part) -> Result<Authors, PptxError> {
    if pres.rels.first_of("commentAuthors").is_some() {
        return Err(PptxError::Unsupported("classic PowerPoint comments; modern comments are required".into()));
    }
    let mut authors = Authors::new();
    let Some(rel) = pres.rels.first_of("authors") else { return Ok(authors) };
    if rel.external || rel.kind_uri != comments::RT_AUTHORS {
        return Err(PptxError::Unsupported("comment authors must use the modern internal author relationship".into()));
    }
    let doc = part(pkg, &rel.target, CT_AUTHORS, "authorLst")?;
    for a in doc.root.children_named("author") {
        let id = required(a, "id")?;
        let name = required(a, "name")?;
        let initials = a.attr("initials").unwrap_or("");
        if authors.insert(id.into(), (name.into(), initials.into())).is_some() {
            return Err(PptxError::Xml(format!("duplicate comment author {id}")));
        }
    }
    Ok(authors)
}

fn required<'a>(el: &'a El, attr: &str) -> Result<&'a str, PptxError> {
    el.attr(attr).ok_or_else(|| PptxError::Xml(format!("comment {} is missing {attr}", el.local())))
}

fn content(el: &El, authors: &Authors, ids: &mut HashSet<String>) -> Result<Comment, PptxError> {
    if el.child("extLst").is_some_and(|ext| ext.elements().next().is_some()) {
        return Err(PptxError::Unsupported("comment extensions (including reactions and mentions)".into()));
    }
    for attr in ["startDate", "dueDate", "assignedTo", "title"] {
        if el.attr(attr).is_some_and(|v| !v.is_empty()) {
            return Err(PptxError::Unsupported(format!("comment task property {attr}")));
        }
    }
    if el.attr("complete").is_some_and(|v| !matches!(v, "0" | "0%")) {
        return Err(PptxError::Unsupported("comment task completion".into()));
    }
    let id = required(el, "id")?;
    if !ids.insert(id.into()) {
        return Err(PptxError::Xml(format!("duplicate comment id {id}")));
    }
    let author_id = required(el, "authorId")?;
    let (author, initials) = authors.get(author_id).ok_or_else(|| PptxError::Xml(format!("missing comment author {author_id}")))?;
    let date = required(el, "created")?;
    if !comments::valid_date_time(date) {
        return Err(PptxError::Xml(format!("comment {id} has an invalid creation date")));
    }
    let resolved = match el.attr("status").unwrap_or("active") {
        "active" => false,
        "resolved" => true,
        status => return Err(PptxError::Unsupported(format!("comment status {status}"))),
    };
    let text = el
        .child("txBody")
        .map(|body| {
            body.children_named("p")
                .map(|p| {
                    p.elements().map(|r| if r.is("br") { "\u{b}".into() } else { r.child("t").map(El::text).unwrap_or_default() }).collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    Ok(Comment { author: author.clone(), initials: initials.clone(), text, date: date.into(), resolved, ..Default::default() })
}

pub(super) fn read(pkg: &Package, path: &str, authors: &Authors, shapes: &IdCtx, slide_id: u32) -> Result<Vec<Comment>, PptxError> {
    let doc = part(pkg, path, CT_COMMENTS, "cmLst")?;
    let mut out = Vec::new();
    let mut ids = HashSet::new();
    for el in doc.root.children_named("cm") {
        if out.len() >= 10_000 {
            return Err(PptxError::Unsupported("more than 10000 comments on one slide".into()));
        }
        if el.elements().filter(|e| matches!(e.local(), "sldMkLst" | "deMkLst" | "txMkLst" | "unknownAnchor")).count() > 1 {
            return Err(PptxError::Unsupported("comment with multiple anchors".into()));
        }
        let mut c = content(el, authors, &mut ids)?;
        if el.child("txMkLst").is_some() {
            return Err(PptxError::Unsupported("comment anchored to a text range".into()));
        }
        if let Some(anchor) = el.child("deMkLst").or_else(|| el.child("sldMkLst")) {
            if anchor.child("sldMk").and_then(|s| s.u32("sldId")) != Some(slide_id) {
                return Err(PptxError::Xml("comment anchor does not identify its slide".into()));
            }
            if anchor.is("deMkLst") {
                let target =
                    anchor.elements().last().filter(|e| matches!(e.local(), "spMk" | "grpSpMk" | "cxnSpMk" | "graphicFrameMk" | "picMk" | "inkMk"));
                let id = target
                    .and_then(|e| e.u32("id"))
                    .and_then(|id| shapes.lookup(id))
                    .ok_or_else(|| PptxError::Xml("comment references a missing shape".into()))?;
                c.shape = Some(id);
            }
        } else if el.child("unknownAnchor").is_some() {
            return Err(PptxError::Unsupported("comment with an unknown anchor".into()));
        } else {
            return Err(PptxError::Xml("comment has no anchor".into()));
        }
        if let Some(pos) = el.child("pos") {
            let coordinate = |attr| -> Result<f64, PptxError> {
                required(pos, attr)?
                    .parse::<i64>()
                    .map(|n| n as f64 / 12700.0)
                    .map_err(|_| PptxError::Xml(format!("invalid comment coordinate {attr}")))
            };
            c.x = coordinate("x")?;
            c.y = coordinate("y")?;
        }
        if let Some(replies) = el.child("replyLst") {
            for reply in replies.children_named("reply") {
                if c.replies.len() >= 1000 || reply.child("replyLst").is_some() {
                    return Err(PptxError::Unsupported("nested replies or more than 1000 replies in a comment thread".into()));
                }
                c.replies.push(content(reply, authors, &mut ids)?);
            }
        }
        out.push(c);
    }
    Ok(out)
}
