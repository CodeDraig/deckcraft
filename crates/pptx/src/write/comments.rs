//! Modern comment threads and their presentation-wide author identities.

use deckcraft_model::{Comment, ShapeKind, Slide};

use super::shapes::IdMap;
use crate::PptxError;
use crate::comments::{self, NS, NS_AC, NS_PC};
use crate::opc::NS_A;
use crate::xml::{A, W};

#[derive(Default)]
pub(super) struct Authors {
    entries: Vec<(String, String)>,
    next_comment: u64,
}

impl Authors {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn properties(&mut self, c: &Comment) -> Result<A, PptxError> {
        if !comments::valid_date_time(&c.date) {
            return Err(PptxError::Write("comment requires a valid ISO 8601 creation date".into()));
        }
        for (name, value) in [("author", &c.author), ("initials", &c.initials), ("text", &c.text)] {
            if !value.chars().all(|ch| crate::xml::xml_char(ch) || (name == "text" && ch == '\u{b}')) {
                return Err(PptxError::Write(format!("comment {name} contains a character forbidden by XML 1.0")));
            }
        }
        let author = self.entries.iter().position(|(name, initials)| name == &c.author && initials == &c.initials).unwrap_or_else(|| {
            self.entries.push((c.author.clone(), c.initials.clone()));
            self.entries.len() - 1
        });
        self.next_comment = self.next_comment.checked_add(1).ok_or_else(|| PptxError::Write("too many comments".into()))?;
        Ok(A::new()
            .a("id", guid(2, self.next_comment))
            .a("authorId", guid(1, author as u64))
            .a("created", &c.date)
            .a("status", if c.resolved { "resolved" } else { "active" }))
    }

    pub fn xml(&self) -> Vec<u8> {
        let mut w = W::new();
        w.open("p188:authorLst", A::new().a("xmlns:p188", NS));
        for (i, (name, initials)) in self.entries.iter().enumerate() {
            let id = guid(1, i as u64);
            w.empty(
                "p188:author",
                A::new().a("id", &id).a("name", name).a("initials", initials).a("userId", format!("deckcraft:{id}")).a("providerId", ""),
            );
        }
        w.close("p188:authorLst");
        w.finish()
    }
}

// IDs are package-local, deterministic and disjoint for authors and comments. The model does
// not retain a remote identity; no account or email address is invented for a local author.
fn guid(kind: u16, index: u64) -> String {
    format!("{{DEC4CAFE-{kind:04X}-4000-8000-{index:012X}}}")
}

fn text(w: &mut W, text: &str) {
    w.open0("p188:txBody");
    w.empty0("a:bodyPr");
    w.empty0("a:lstStyle");
    for para in text.split('\n') {
        w.open0("a:p");
        for (i, line) in para.split('\u{b}').enumerate() {
            if i != 0 {
                w.empty0("a:br");
            }
            w.open0("a:r");
            w.open("a:t", A::new().a("xml:space", "preserve"));
            // Literal carriage returns are normalized by XML readers; preserve them as references.
            w.raw(&crate::xml::esc(line).replace('\r', "&#13;"));
            w.close("a:t");
            w.close("a:r");
        }
        w.close("a:p");
    }
    w.close("p188:txBody");
}

pub(super) fn slide_xml(slide: &Slide, slide_id: u32, ids: &IdMap, authors: &mut Authors) -> Result<Vec<u8>, PptxError> {
    if slide.comments.len() > 10_000 {
        return Err(PptxError::Unsupported("more than 10000 comments on one slide".into()));
    }
    let mut w = W::new();
    w.open("p188:cmLst", A::new().a("xmlns:p188", NS).a("xmlns:a", NS_A).a("xmlns:pc", NS_PC).a("xmlns:ac", NS_AC));
    for c in &slide.comments {
        w.open("p188:cm", authors.properties(c)?);
        let anchor = if c.shape.is_some() { "ac:deMkLst" } else { "pc:sldMkLst" };
        w.open0(anchor);
        w.empty0("pc:docMk");
        w.empty("pc:sldMk", A::new().a("sldId", slide_id));
        if let Some(id) = c.shape {
            let shape = slide.shape(id).ok_or_else(|| PptxError::Write(format!("comment references missing shape {id}")))?;
            let file_id = ids.get(id).ok_or_else(|| PptxError::Write(format!("comment shape {id} has no exported id")))?;
            let tag = match shape.kind {
                ShapeKind::Shape => "ac:spMk",
                ShapeKind::Picture { .. } | ShapeKind::Media(_) => "ac:picMk",
                ShapeKind::Group { .. } | ShapeKind::Ink { .. } => "ac:grpSpMk",
                ShapeKind::Connector { .. } => "ac:cxnSpMk",
                ShapeKind::Table(_) | ShapeKind::Chart(_) => "ac:graphicFrameMk",
                ShapeKind::Opaque { .. } => return Err(PptxError::Unsupported("comment anchor on an opaque shape".into())),
            };
            w.empty(tag, A::new().a("id", file_id));
        }
        w.close(anchor);
        if !c.x.is_finite() || !c.y.is_finite() || c.x.abs() > 1.0e10 || c.y.abs() > 1.0e10 {
            return Err(PptxError::Write("comment position is out of range".into()));
        }
        w.empty("p188:pos", A::new().a("x", (c.x * 12700.0).round() as i64).a("y", (c.y * 12700.0).round() as i64));
        if c.replies.len() > 1000 {
            return Err(PptxError::Unsupported("more than 1000 replies in a comment thread".into()));
        }
        if !c.replies.is_empty() {
            w.open0("p188:replyLst");
            for reply in &c.replies {
                if !reply.replies.is_empty() || reply.shape.is_some() || reply.x != 0.0 || reply.y != 0.0 {
                    return Err(PptxError::Unsupported("modern comment replies cannot have replies or independent anchors/positions".into()));
                }
                w.open("p188:reply", authors.properties(reply)?);
                text(&mut w, &reply.text);
                w.close("p188:reply");
            }
            w.close("p188:replyLst");
        }
        text(&mut w, &c.text);
        w.close("p188:cm");
    }
    w.close("p188:cmLst");
    Ok(w.finish())
}

pub(super) fn relationship(w: &mut W, rid: &str) {
    w.open("p:ext", A::new().a("uri", comments::EXT_URI));
    w.empty("p188:commentRel", A::new().a("xmlns:p188", NS).a("r:id", rid));
    w.close("p:ext");
}
