//! Caret navigation and text hit testing over positioned lines.

use super::{LineInfo, TextLayout};
use deckcraft_geom::{Point, Rect};

/// A caret position: paragraph and character offset within it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub para: usize,
    pub ch: usize,
}

impl TextLayout {
    /// The line containing a caret position (the later line at a soft wrap boundary).
    pub fn line_of(&self, pos: Pos) -> Option<usize> {
        let mut found = None;
        for (i, l) in self.lines.iter().enumerate() {
            if l.para == pos.para && pos.ch >= l.start && pos.ch <= l.end {
                found = Some(i);
                if pos.ch < l.end {
                    break;
                }
            }
        }
        found.or_else(|| self.lines.iter().rposition(|l| l.para == pos.para))
    }
    /// Caret line segment (x, top, bottom) in shape-local, unrotated coordinates.
    pub fn caret(&self, pos: Pos) -> Option<(f64, f64, f64)> {
        let li = self.line_of(pos)?;
        let l = self.lines.get(li)?;
        let k = pos.ch.clamp(l.start, l.end) - l.start;
        let x = l.caret_x.get(k).copied().or(l.caret_x.last().copied())?;
        Some((x, l.top, l.bottom))
    }
    /// Nearest caret position for a point.
    pub fn hit(&self, p: Point) -> Pos {
        let mut best: Option<&LineInfo> = None;
        for l in &self.lines {
            if p.y >= l.top && p.y < l.bottom {
                // In multi-column bodies pick the line whose column contains x.
                if best.is_none() || l.caret_x.first().is_some_and(|x0| p.x >= *x0 - 4.0) {
                    best = Some(l);
                }
            }
        }
        let l = match best {
            Some(l) => l,
            None => {
                if self.lines.is_empty() {
                    return Pos::default();
                }
                let first = self.lines.first();
                let last = self.lines.last();
                match (first, last) {
                    (Some(f), _) if p.y < f.top => f,
                    (_, Some(l)) => l,
                    _ => return Pos::default(),
                }
            }
        };
        let mut idx = l.end;
        for k in 0..l.caret_x.len().saturating_sub(1) {
            let (a, b) = (l.caret_x.get(k).copied().unwrap_or(0.0), l.caret_x.get(k + 1).copied().unwrap_or(0.0));
            if p.x < (a + b) / 2.0 {
                idx = l.start + k;
                break;
            }
        }
        Pos { para: l.para, ch: idx }
    }
    /// Selection highlight rectangles between two positions.
    pub fn selection_rects(&self, a: Pos, b: Pos) -> Vec<Rect> {
        let (s, e) = if a <= b { (a, b) } else { (b, a) };
        let mut out = vec![];
        for l in &self.lines {
            let lp = Pos { para: l.para, ch: l.start };
            let le = Pos { para: l.para, ch: l.end };
            if le < s || lp > e {
                continue;
            }
            let from = if s > lp { s.ch - l.start } else { 0 };
            let to = if e < le { e.ch.saturating_sub(l.start) } else { l.end - l.start };
            let x0 = l.caret_x.get(from).copied().unwrap_or(0.0);
            let mut x1 = l.caret_x.get(to).copied().unwrap_or(x0);
            // Selecting across a paragraph end shows a small extra box for the newline.
            if e.para > l.para && l.end == to + l.start {
                x1 += 6.0;
            }
            if x1 > x0 {
                out.push(Rect::new(x0, l.top, x1, l.bottom));
            }
        }
        out
    }
    /// Vertical caret movement: the position on the line above/below closest to `x`.
    pub fn vertical(&self, pos: Pos, down: bool, x: f64) -> Pos {
        let Some(li) = self.line_of(pos) else { return pos };
        let target = if down { li + 1 } else { li.wrapping_sub(1) };
        let Some(l) = self.lines.get(target) else { return pos };
        let p = Point::new(x, (l.top + l.bottom) / 2.0);
        let mut h = self.hit(p);
        if h.para != l.para {
            h.para = l.para;
            h.ch = h.ch.clamp(l.start, l.end);
        }
        h
    }
    /// Start/end of the visual line containing `pos`.
    pub fn line_bounds(&self, pos: Pos) -> (Pos, Pos) {
        match self.line_of(pos).and_then(|i| self.lines.get(i)) {
            Some(l) => (Pos { para: l.para, ch: l.start }, Pos { para: l.para, ch: l.end }),
            None => (pos, pos),
        }
    }
    /// Bounding box of all glyphs/lines (shape-local).
    pub fn bounds(&self) -> Rect {
        let mut r: Option<Rect> = None;
        for l in &self.lines {
            let x0 = l.caret_x.first().copied().unwrap_or(0.0);
            let x1 = l.caret_x.last().copied().unwrap_or(x0);
            let lr = Rect::new(x0, l.top, x1.max(x0), l.bottom);
            r = Some(r.map(|a| a.union(lr)).unwrap_or(lr));
        }
        r.unwrap_or(self.inner)
    }
}
