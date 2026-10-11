//! Modern-comment package contracts, independent fixtures and failed-import preservation.

use std::io::{Cursor, Read, Write};
use std::sync::Arc;

use deckcraft_model::{Comment, Presentation, ShapeId};
use deckcraft_pptx::{export, import};
use serde_json::json;

fn deck() -> Presentation {
    let mut p = Presentation::default();
    let s = Arc::make_mut(&mut p.slides[0]);
    s.comments.push(Comment {
        author: "Ada & Co".into(),
        initials: "AC".into(),
        text: " First <line>\nSecond\u{b}line 日本語 \r\tend".into(),
        date: "2026-10-10T12:34:56.123-04:00".into(),
        x: 12.5,
        y: -7.25,
        resolved: true,
        shape: Some(s.shapes[0].id),
        replies: vec![Comment {
            author: "Ada & Co".into(),
            initials: "AD".into(),
            text: "Reply".into(),
            date: "2026-10-10T17:00:00Z".into(),
            ..Default::default()
        }],
    });
    p
}

fn entry(bytes: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = String::new();
    z.by_name(name).unwrap().read_to_string(&mut out).unwrap();
    out
}

fn replace(bytes: &[u8], name: &str, contents: Option<&str>) -> Vec<u8> {
    let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..source.len() {
        let mut part = source.by_index(i).unwrap();
        if part.name() == name && contents.is_none() {
            continue;
        }
        out.start_file(part.name(), zip::write::SimpleFileOptions::default()).unwrap();
        if part.name() == name {
            out.write_all(contents.unwrap().as_bytes()).unwrap();
        } else {
            std::io::copy(&mut part, &mut out).unwrap();
        }
    }
    out.finish().unwrap().into_inner()
}

#[test]
fn threads_authors_positions_and_anchors_round_trip() {
    let p = deck();
    let bytes = export(&p).unwrap();
    let q = import(&bytes).unwrap();
    assert_eq!(p.slides[0].comments, q.slides[0].comments);
    assert_eq!(q.slides[0].comments, import(&export(&q).unwrap()).unwrap().slides[0].comments);
    let types = entry(&bytes, "[Content_Types].xml");
    assert!(types.contains("application/vnd.ms-powerpoint.comments+xml"));
    assert!(types.contains("application/vnd.ms-powerpoint.authors+xml"));
    assert!(!types.contains("presentationml.comments"));
    assert!(!types.contains("commentAuthors"));
    assert!(entry(&bytes, "ppt/_rels/presentation.xml.rels").contains("http://schemas.microsoft.com/office/2018/10/relationships/authors"));
    assert!(entry(&bytes, "ppt/slides/_rels/slide1.xml.rels").contains("http://schemas.microsoft.com/office/2018/10/relationships/comments"));
    assert!(entry(&bytes, "ppt/slides/slide1.xml").contains("p188:commentRel"));
    let xml = entry(&bytes, "ppt/comments/comment1.xml");
    assert!(xml.contains("<p188:replyLst>"));
    assert!(xml.contains("<ac:spMk"));
    assert!(xml.contains("x=\"158750\" y=\"-92075\""));
}

#[test]
fn independent_modern_fixture_with_arbitrary_prefixes() {
    let bytes = export(&deck()).unwrap();
    let authors = r#"<m:authorLst xmlns:m="http://schemas.microsoft.com/office/powerpoint/2018/8/main"><m:author id="{11111111-1111-4111-8111-111111111111}" name="Marie" initials="M" userId="local" providerId=""/></m:authorLst>"#;
    let comments = r#"<m:cmLst xmlns:m="http://schemas.microsoft.com/office/powerpoint/2018/8/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pc="http://schemas.microsoft.com/office/powerpoint/2013/main/command"><m:cm id="{22222222-2222-4222-8222-222222222222}" authorId="{11111111-1111-4111-8111-111111111111}" created="2026-01-02T03:04:05Z"><pc:sldMkLst><pc:docMk/><pc:sldMk sldId="256"/></pc:sldMkLst><m:pos x="127000" y="254000"/><m:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Hello</a:t></a:r><a:br/><a:r><a:t>world</a:t></a:r></a:p></m:txBody></m:cm></m:cmLst>"#;
    let bytes = replace(&replace(&bytes, "ppt/authors.xml", Some(authors)), "ppt/comments/comment1.xml", Some(comments));
    let p = import(&bytes).unwrap();
    assert_eq!(
        p.slides[0].comments,
        vec![Comment {
            author: "Marie".into(),
            initials: "M".into(),
            text: "Hello\u{b}world".into(),
            date: "2026-01-02T03:04:05Z".into(),
            x: 10.0,
            y: 20.0,
            ..Default::default()
        }]
    );
}

#[test]
fn shape_anchors_follow_reassigned_file_ids() {
    let mut p = deck();
    let slide = Arc::make_mut(&mut p.slides[0]);
    slide.shapes[0].id = ShapeId(u32::MAX);
    slide.comments[0].shape = Some(ShapeId(u32::MAX));
    let q = import(&export(&p).unwrap()).unwrap();
    assert_eq!(q.slides[0].comments[0].shape, Some(q.slides[0].shapes[0].id));
}

#[test]
fn unsupported_export_preserves_source() {
    let mut p = deck();
    Arc::make_mut(&mut p.slides[0]).comments[0].replies[0].replies.push(Comment::default());
    let before = p.clone();
    assert!(export(&p).unwrap_err().to_string().contains("replies"));
    assert_eq!(p, before);
    Arc::make_mut(&mut p.slides[0]).comments[0].replies.clear();
    Arc::make_mut(&mut p.slides[0]).comments[0].date.clear();
    assert!(export(&p).unwrap_err().to_string().contains("creation date"));
    for field in ["author", "initials", "text"] {
        let mut p = deck();
        let c = &mut Arc::make_mut(&mut p.slides[0]).comments[0];
        match field {
            "author" => c.author.push('\0'),
            "initials" => c.initials.push('\0'),
            _ => c.text.push('\0'),
        }
        let before = p.clone();
        assert!(export(&p).unwrap_err().to_string().contains("forbidden by XML"));
        assert_eq!(p, before);
    }
}

#[test]
fn damaged_or_classic_comments_fail_without_replacing_active_document() {
    let bytes = export(&deck()).unwrap();
    let types = entry(&bytes, "[Content_Types].xml")
        .replace("application/vnd.ms-powerpoint.comments+xml", "application/vnd.openxmlformats-officedocument.presentationml.comments+xml");
    let classic = replace(&bytes, "[Content_Types].xml", Some(&types));
    let wrong_ns = entry(&bytes, "ppt/comments/comment1.xml")
        .replace("http://schemas.microsoft.com/office/powerpoint/2018/8/main", "http://schemas.openxmlformats.org/presentationml/2006/main");
    let missing_author = entry(&bytes, "ppt/authors.xml").replace("DEC4CAFE", "DEADC0DE");
    let xml = entry(&bytes, "ppt/comments/comment1.xml");
    let task = xml.replace("<p188:cm ", "<p188:cm dueDate=\"2026-10-12T00:00:00Z\" ");
    let reaction = xml.replace(
        "</p188:cm>",
        "<p188:extLst><p:ext xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" uri=\"reaction\"/></p188:extLst></p188:cm>",
    );
    let unknown_anchor = xml.replace("<ac:deMkLst>", "<p188:unknownAnchor>").replace("</ac:deMkLst>", "</p188:unknownAnchor>");
    let multiple_anchors = xml.replace("</ac:deMkLst>", "</ac:deMkLst><p188:unknownAnchor/>");
    let cases = [
        classic,
        replace(&bytes, "ppt/comments/comment1.xml", Some(&unknown_anchor)),
        replace(&bytes, "ppt/comments/comment1.xml", Some(&multiple_anchors)),
        replace(&bytes, "ppt/comments/comment1.xml", Some(&task)),
        replace(&bytes, "ppt/comments/comment1.xml", Some(&reaction)),
        replace(&bytes, "ppt/comments/comment1.xml", Some(&wrong_ns)),
        replace(&bytes, "ppt/comments/comment1.xml", None),
        replace(&bytes, "ppt/authors.xml", Some(&missing_author)),
    ];
    for bytes in cases {
        assert!(import(&bytes).is_err());
        let mut session = deckcraft_engine::Session::with_new();
        session.execute("insert.textBox", &json!({"text":"Keep me"})).unwrap();
        let before = session.active().unwrap().doc.clone();
        let count = session.documents().len();
        let data = deckcraft_engine::cmd::base64_encode(&bytes);
        assert!(session.execute("file.openBytes", &json!({"name":"bad.pptx", "data":data})).is_err());
        assert!(Arc::ptr_eq(&before, &session.active().unwrap().doc));
        assert_eq!(session.documents().len(), count);
    }
}
