//! Greedy paragraph line breaking and tab advances.

use super::shaping::ParaShaped;

/// Break opportunities: allowed[i] = a line may start at char i.
pub(super) fn break_opportunities(text: &str) -> (Vec<bool>, Vec<bool>) {
    let n = text.chars().count();
    let mut allowed = vec![false; n + 1];
    let mut mandatory = vec![false; n + 1];
    let mut byte_to_char = std::collections::HashMap::new();
    for (ci, (b, _)) in text.char_indices().enumerate() {
        byte_to_char.insert(b, ci);
    }
    byte_to_char.insert(text.len(), n);
    for (b, op) in unicode_linebreak::linebreaks(text) {
        if let Some(&ci) = byte_to_char.get(&b) {
            if let Some(a) = allowed.get_mut(ci) {
                *a = true;
            }
            if op == unicode_linebreak::BreakOpportunity::Mandatory
                && let Some(m) = mandatory.get_mut(ci)
            {
                *m = true;
            }
        }
    }
    (allowed, mandatory)
}

pub(super) struct RawLine {
    pub(super) start: usize,
    pub(super) end: usize,
    /// End including trailing spaces / the break char consumed.
    pub(super) next: usize,
}

pub(super) fn break_lines(
    p: &ParaShaped,
    first_w: f64,
    rest_w: f64,
    wrap: bool,
    tab_stops: &[f64],
    default_tab: f64,
    x0_first: f64,
    x0_rest: f64,
) -> Vec<RawLine> {
    let text: String = p.cells.iter().map(|c| c.ch).collect();
    let (allowed, _) = break_opportunities(&text);
    let n = p.cells.len();
    let mut lines = vec![];
    let mut start = 0;
    if n == 0 {
        return vec![RawLine { start: 0, end: 0, next: 0 }];
    }
    while start < n {
        let avail = if lines.is_empty() { first_w } else { rest_w };
        let x0 = if lines.is_empty() { x0_first } else { x0_rest };
        let mut x = 0.0;
        let mut last_break: Option<usize> = None;
        let mut i = start;
        let mut end = n;
        let mut next = n;
        while i < n {
            let c = p.cells.get(i).map(|c| c.ch).unwrap_or(' ');
            if c == '\u{b}' || c == '\n' {
                end = i;
                next = i + 1;
                break;
            }
            let adv = if c == '\t' { tab_advance(x0 + x, tab_stops, default_tab) } else { p.cells.get(i).map(|c| c.adv).unwrap_or(0.0) };
            if i > start && allowed.get(i).copied().unwrap_or(false) {
                last_break = Some(i);
            }
            if wrap && x + adv > avail + 0.01 && !c.is_whitespace() && i > start {
                // Break at the last opportunity, or force mid-word.
                let b = last_break.filter(|b| *b > start).unwrap_or(i);
                end = b;
                next = b;
                // Trailing spaces belong to the line but don't count.
                while end > start && p.cells.get(end - 1).is_some_and(|c| c.ch == ' ') {
                    end -= 1;
                }
                break;
            }
            x += adv;
            i += 1;
        }
        if i >= n {
            end = n;
            next = n;
        }
        lines.push(RawLine { start, end, next });
        if next <= start {
            // No progress: force one char.
            let last = lines.len() - 1;
            if let Some(l) = lines.get_mut(last) {
                l.end = (start + 1).min(n);
                l.next = l.end;
            }
            start += 1;
        } else {
            start = next;
        }
        if next == n && p.cells.last().is_some_and(|c| c.ch == '\u{b}') && start >= n {
            // A trailing line break opens an empty last line.
            lines.push(RawLine { start: n, end: n, next: n });
        }
    }
    lines
}

pub(super) fn tab_advance(x: f64, stops: &[f64], default_tab: f64) -> f64 {
    for s in stops {
        if *s > x + 0.5 {
            return s - x;
        }
    }
    let d = if default_tab > 1.0 { default_tab } else { 72.0 };
    let next = ((x / d).floor() + 1.0) * d;
    (next - x).max(1.0)
}
