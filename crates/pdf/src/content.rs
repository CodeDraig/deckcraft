//! Slide artwork, searchable text and page annotations.

use super::{PageLayout, PdfOptions};
use deckcraft_fonts::FontDb;
use deckcraft_model::Presentation;
use deckcraft_model::text::Action as ClickAction;
use deckcraft_render::{PlacedText, RenderOpts};
use krilla::action::{Action, LinkAction};
use krilla::annotation::{Annotation, LinkAnnotation, Target};
use krilla::destination::XyzDestination;
use krilla::geom::{Point, Size, Transform};
use krilla::image::Image;
use krilla::num::NormalizedF32;
use krilla::paint::Fill;
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph, TextDirection};
use kurbo::{Affine, Rect};
use std::collections::HashMap;

pub(super) struct Exporter<'a> {
    pub(super) pres: &'a Presentation,
    pub(super) opts: &'a PdfOptions,
    pub(super) fonts: HashMap<u32, Option<Font>>,
}

pub(super) fn tf(a: Affine) -> Transform {
    let c = a.as_coeffs();
    Transform::from_row(c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32, c[4] as f32, c[5] as f32)
}

pub(super) fn grey(v: u8, opacity: f32) -> Fill {
    Fill {
        paint: krilla::color::rgb::Color::new(v, v, v).into(),
        opacity: NormalizedF32::new(opacity.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE),
        ..Default::default()
    }
}

impl Exporter<'_> {
    fn font(&mut self, face: &deckcraft_fonts::FontFace) -> Option<Font> {
        self.fonts.entry(face.id()).or_insert_with(|| Font::new(face.data().to_vec().into(), face.index())).clone()
    }

    /// Slide `i` drawn into `b` (page points).
    pub(super) fn slide(&mut self, s: &mut Surface, i: usize, b: Rect) {
        let pres = self.pres;
        let (sw, sh) = (pres.slide_size.width.max(1.0), pres.slide_size.height.max(1.0));
        let k = b.width() / sw;
        // Raster at the requested dpi relative to the box size on paper.
        let scale = (b.width() / 72.0 * self.opts.dpi.clamp(36.0, 600.0)) / sw;
        let img = deckcraft_render::render_slide(pres, i, &RenderOpts { scale, threads: 4, ..Default::default() });
        if img.width > 0 && img.height > 0 {
            let mut rgba = img.pixels.clone();
            for px in rgba.as_chunks_mut::<4>().0 {
                // Composite onto white (slides are normally opaque).
                let a = px[3] as u32;
                for c in &mut px[..3] {
                    *c = (*c as u32 + (255 - a)).min(255) as u8;
                }
                px[3] = 255;
            }
            let image = Image::from_rgba8(rgba, img.width, img.height);
            if let Some(size) = Size::from_wh(b.width() as f32, b.height() as f32) {
                s.push_transform(&Transform::from_translate(b.x0 as f32, b.y0 as f32));
                s.draw_image(image, size);
                s.pop();
            }
        }
        if self.opts.frame_slides && self.opts.layout != PageLayout::Slides {
            let p = kurbo::Shape::to_path(&b, 0.1);
            if let Some(path) = to_path(&p) {
                s.set_fill(None);
                s.set_stroke(Some(krilla::paint::Stroke {
                    paint: krilla::color::rgb::Color::new(0x80, 0x80, 0x80).into(),
                    width: 0.5,
                    ..Default::default()
                }));
                s.draw_path(&path);
                s.set_stroke(None);
            }
        }
        if self.opts.text_layer {
            let view = Affine::translate((b.x0, b.y0)) * Affine::scale(k);
            for pt in deckcraft_render::place_slide(pres, i).texts {
                self.text_layer(s, &pt, view);
            }
            let _ = sh;
        }
    }

    /// Invisible glyphs exactly over the rasterised text.
    fn text_layer(&mut self, s: &mut Surface, pt: &PlacedText, view: Affine) {
        for run in &pt.layout.runs {
            if run.glyphs.is_empty() || run.size <= 0.0 {
                continue;
            }
            let text: String = pt
                .body
                .paragraphs
                .get(run.para)
                .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
                .unwrap_or_default()
                .chars()
                .skip(run.chars.0)
                .take(run.chars.1.saturating_sub(run.chars.0))
                .collect();
            if text.trim().is_empty() {
                continue;
            }
            let Some(font) = self.font(&run.face) else { continue };
            let n = run.glyphs.len();
            let char_starts: Vec<usize> = text.char_indices().map(|(b, _)| b).chain(std::iter::once(text.len())).collect();
            let one_to_one = char_starts.len() - 1 == n;
            let (x0, y0) = (run.glyphs[0].1, run.glyphs[0].2);
            let kg: Vec<KrillaGlyph> = run
                .glyphs
                .iter()
                .enumerate()
                .map(|(gi, g)| {
                    let next = run.glyphs.get(gi + 1).map_or(g.1 + run.face.advance(g.0) * run.size / run.face.upem.max(1.0), |nx| nx.1);
                    let range = if one_to_one {
                        char_starts[gi]..char_starts[gi + 1]
                    } else if gi == 0 {
                        0..text.len()
                    } else {
                        text.len()..text.len()
                    };
                    KrillaGlyph::new(GlyphId::new(g.0), ((next - g.1) / run.size) as f32, 0.0, ((g.2 - y0) / run.size) as f32, 0.0, range, None)
                })
                .collect();
            s.push_transform(&tf(view * pt.transform * Affine::translate((x0, y0))));
            s.set_fill(Some(grey(0, 0.0)));
            s.draw_glyphs(Point::from_xy(0.0, 0.0), &kg, font, &text, run.size as f32, false);
            s.set_fill(None);
            s.pop();
        }
    }

    pub(super) fn links(&self, i: usize, b: Rect, page_of: &HashMap<usize, usize>) -> Vec<Annotation> {
        let pres = self.pres;
        let k = b.width() / pres.slide_size.width.max(1.0);
        let view = Affine::translate((b.x0, b.y0)) * Affine::scale(k);
        let mut out = Vec::new();
        for l in deckcraft_render::place_slide(pres, i).links {
            let r = view.transform_rect_bbox(l.rect).intersect(b);
            let Some(rect) = krilla::geom::Rect::from_ltrb(r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32) else { continue };
            let goto = |slide: Option<usize>| {
                slide.and_then(|s| page_of.get(&s)).map(|p| Target::Destination(XyzDestination::new(*p, Point::from_xy(0.0, 0.0)).into()))
            };
            let target = match &l.link.action {
                ClickAction::Url { url } => Some(Target::Action(Action::Link(LinkAction::new(url.clone())))),
                ClickAction::Slide { slide } => goto(pres.slides.iter().position(|x| x.id == *slide)),
                ClickAction::NextSlide => goto(Some(i + 1)),
                ClickAction::PreviousSlide => goto(i.checked_sub(1)),
                ClickAction::FirstSlide => goto(Some(0)),
                ClickAction::LastSlide => goto(pres.slides.len().checked_sub(1)),
                _ => None,
            };
            if let Some(t) = target {
                let alt = if l.link.tooltip.is_empty() { None } else { Some(l.link.tooltip.clone()) };
                out.push(Annotation::new_link(LinkAnnotation::new(rect, t), alt));
            }
        }
        out
    }

    fn ui_font(&mut self) -> Option<(Font, std::sync::Arc<deckcraft_fonts::FontFace>)> {
        let face = FontDb::global().face(deckcraft_fonts::DEFAULT_FAMILY, "Regular");
        let f = self.font(&face)?;
        Some((f, face))
    }

    /// Wrapped plain text (notes).
    pub(super) fn paragraphs(&mut self, s: &mut Surface, text: &str, area: Rect, size: f64) {
        let Some((font, face)) = self.ui_font() else { return };
        let k = size / face.upem.max(1.0);
        let width = |w: &str| w.chars().map(|c| face.advance(face.glyph_for(c)) * k).sum::<f64>();
        let lh = size * 1.3;
        let mut y = area.y0 + size;
        s.set_fill(Some(grey(0x20, 1.0)));
        'outer: for para in text.split('\n') {
            let mut line = String::new();
            for word in para.split(' ') {
                let cand = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                if width(&cand) > area.width() && !line.is_empty() {
                    if y > area.y1 {
                        break 'outer;
                    }
                    s.draw_text(Point::from_xy(area.x0 as f32, y as f32), font.clone(), size as f32, &line, false, TextDirection::Auto);
                    y += lh;
                    line = word.to_string();
                } else {
                    line = cand;
                }
            }
            if y > area.y1 {
                break;
            }
            if !line.is_empty() {
                s.draw_text(Point::from_xy(area.x0 as f32, y as f32), font.clone(), size as f32, &line, false, TextDirection::Auto);
            }
            y += lh;
        }
        s.set_fill(None);
    }

    /// Ruled lines for handwritten notes (3-per-page handouts).
    pub(super) fn note_lines(&self, s: &mut Surface, r: Rect) {
        let mut y = r.y0 + 18.0;
        s.set_stroke(Some(krilla::paint::Stroke {
            paint: krilla::color::rgb::Color::new(0xB0, 0xB0, 0xB0).into(),
            width: 0.5,
            ..Default::default()
        }));
        while y <= r.y1 {
            let mut p = kurbo::BezPath::new();
            p.move_to((r.x0, y));
            p.line_to((r.x1, y));
            if let Some(path) = to_path(&p) {
                s.draw_path(&path);
            }
            y += 22.0;
        }
        s.set_stroke(None);
    }

    pub(super) fn footer(&mut self, s: &mut Surface, pw: f64, ph: f64, n: usize) {
        let Some((font, _)) = self.ui_font() else { return };
        s.set_fill(Some(grey(0x60, 1.0)));
        s.draw_text(Point::from_xy((pw - 72.0) as f32, (ph - 30.0) as f32), font, 9.0, &n.to_string(), false, TextDirection::Auto);
        s.set_fill(None);
    }
}

pub(super) fn to_path(bp: &kurbo::BezPath) -> Option<krilla::geom::Path> {
    use kurbo::PathEl;
    let mut pb = krilla::geom::PathBuilder::new();
    for el in bp.elements() {
        match *el {
            PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
            PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
            PathEl::QuadTo(a, p) => pb.quad_to(a.x as f32, a.y as f32, p.x as f32, p.y as f32),
            PathEl::CurveTo(a, b, p) => pb.cubic_to(a.x as f32, a.y as f32, b.x as f32, b.y as f32, p.x as f32, p.y as f32),
            PathEl::ClosePath => pb.close(),
        }
    }
    pb.finish()
}
