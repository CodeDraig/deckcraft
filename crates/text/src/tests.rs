use super::*;
use deckcraft_geom::Point;
use deckcraft_model::defaults;
use deckcraft_model::text::{Paragraph, Run};
use deckcraft_model::{LayoutType, Presentation, Xfrm};

fn setup(text: &str, w: f64, h: f64) -> (Presentation, Shape) {
    let mut p = Presentation::default();
    let lay = defaults::layout_of_kind(&p, LayoutType::TitleAndContent).unwrap();
    let mut s = defaults::new_slide(&mut p, lay);
    let mut body = s.shapes.remove(1);
    body.xfrm = Some(Xfrm::new(0.0, 0.0, w, h));
    body.text = Some(TextBody::from_text(text));
    s.shapes = vec![body.clone()];
    p.slides = vec![std::sync::Arc::new(s)];
    (p, body)
}

fn lay(p: &Presentation, sh: &Shape, w: f64, h: f64) -> TextLayout {
    let ctx = Ctx::for_slide(p, &p.slides[0]).unwrap();
    layout(&ctx, sh, sh.text.as_ref().unwrap(), &Opts { rect: Rect::new(0.0, 0.0, w, h), fields: &NoFields, prompt_color: None, no_shrink: false })
}

#[test]
fn wraps_long_text_into_lines() {
    let text = "The quick brown fox jumps over the lazy dog and keeps on running far away";
    let (p, sh) = setup(text, 200.0, 400.0);
    let l = lay(&p, &sh, 200.0, 400.0);
    assert!(l.lines.len() >= 3, "{} lines", l.lines.len());
    for li in &l.lines {
        assert!(li.caret_x.last().unwrap() <= &(200.0 + 0.5), "line too wide: {:?}", li.caret_x.last());
        assert_eq!(li.caret_x.len(), li.end - li.start + 1);
    }
    // Lines cover the text without gaps (spaces at wraps are skipped).
    assert_eq!(l.lines[0].start, 0);
    assert!(!l.runs.is_empty());
}

#[test]
fn bullets_are_drawn_for_body_text() {
    let (p, sh) = setup("One\nTwo", 600.0, 300.0);
    let l = lay(&p, &sh, 600.0, 300.0);
    // Two text runs + two bullet runs.
    assert!(l.runs.len() >= 4, "{:?}", l.runs);
    assert_eq!(l.para_lines.len(), 2);
}

#[test]
fn shrink_on_overflow() {
    let text = (0..30).map(|i| format!("Line number {i}")).collect::<Vec<_>>().join("\n");
    let (p, sh) = setup(&text, 600.0, 200.0);
    let l = lay(&p, &sh, 600.0, 200.0);
    assert!(l.font_scale < 1.0, "scale {}", l.font_scale);
    assert!(!l.overflow || l.font_scale <= 0.11);
}

#[test]
fn caret_and_hit_roundtrip() {
    let (p, sh) = setup("Hello world", 600.0, 100.0);
    let l = lay(&p, &sh, 600.0, 100.0);
    for ch in 0..=11 {
        let pos = Pos { para: 0, ch };
        let (x, top, bot) = l.caret(pos).unwrap();
        let back = l.hit(Point::new(x + 0.1, (top + bot) / 2.0));
        assert_eq!(back, pos, "at {ch}");
    }
    let r = l.selection_rects(Pos { para: 0, ch: 0 }, Pos { para: 0, ch: 5 });
    assert_eq!(r.len(), 1);
}

#[test]
fn alignment_center_and_right() {
    let (p, mut sh) = setup("Hi", 400.0, 100.0);
    sh.text.as_mut().unwrap().paragraphs[0].props.align = Some(Align::Center);
    sh.text.as_mut().unwrap().paragraphs[0].props.bullet = Some(Bullet::None);
    let c = lay(&p, &sh, 400.0, 100.0);
    sh.text.as_mut().unwrap().paragraphs[0].props.align = Some(Align::Right);
    let r = lay(&p, &sh, 400.0, 100.0);
    let cx = c.lines[0].caret_x[0];
    let rx = r.lines[0].caret_x[0];
    assert!(rx > cx && cx > 100.0, "center {cx} right {rx}");
}

#[test]
fn empty_and_hostile_bodies_never_panic() {
    let (p, mut sh) = setup("", 10.0, 10.0);
    let _ = lay(&p, &sh, 10.0, 10.0);
    let _ = lay(&p, &sh, 0.0, 0.0);
    let t = sh.text.as_mut().unwrap();
    t.paragraphs = vec![Paragraph {
        runs: vec![
            Run::new("\t\u{b}\u{b}🙂 ﷽ 日本語"),
            Run { text: String::new(), props: Default::default(), kind: deckcraft_model::text::RunKind::Break },
        ],
        ..Default::default()
    }];
    t.paragraphs[0].props.margin_left = Some(-50.0);
    t.paragraphs[0].props.indent = Some(f64::NAN);
    t.paragraphs[0].props.line_spacing = Some(Spacing::Pct(-3.0));
    t.body.columns = Some(1000);
    let l = lay(&p, &sh, 50.0, 50.0);
    let _ = l.hit(Point::new(-100.0, 1e9));
    let _ = l.caret(Pos { para: 99, ch: 99 });
    let _ = l.selection_rects(Pos { para: 5, ch: 0 }, Pos { para: 0, ch: 3 });
}

#[test]
fn autonum_formats() {
    assert_eq!(autonum("arabicPeriod", 3), "3.");
    assert_eq!(autonum("romanUcPeriod", 4), "IV.");
    assert_eq!(autonum("alphaLcParenR", 2), "b)");
    assert_eq!(autonum("arabicParenBoth", 1), "(1)");
    assert_eq!(autonum("alphaUcPeriod", 27), "AA.");
}

#[test]
fn numbered_list_counts() {
    let (p, mut sh) = setup("a\nb\nc", 400.0, 300.0);
    for para in &mut sh.text.as_mut().unwrap().paragraphs {
        para.props.bullet = Some(Bullet::AutoNum { scheme: "arabicPeriod".into(), start_at: 1 });
    }
    let l = lay(&p, &sh, 400.0, 300.0);
    assert!(l.runs.len() >= 6);
}
