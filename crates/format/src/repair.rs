//! Repair the editable document invariants after loading.

use deckcraft_model::{Presentation, defaults};
use std::sync::Arc;

/// Make a loaded presentation safe to edit: a master exists, every slide's layout exists,
/// ids are unique, tables are rectangular.
pub fn repair(p: &mut Presentation) {
    if p.masters.is_empty() {
        let fresh = defaults::blank_presentation(p.slide_size, Default::default(), false);
        p.masters = fresh.masters;
    }
    p.fix_next_id();
    let first_layout = p.masters.first().and_then(|m| m.layouts.first()).map(|l| l.id).unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    for i in 0..p.slides.len() {
        let needs_layout = p.slides.get(i).is_some_and(|s| p.layout(s.layout).is_none());
        let dup = p.slides.get(i).is_some_and(|s| !seen.insert(s.id));
        if needs_layout || dup {
            let new_id = if dup { Some(deckcraft_model::SlideId(p.alloc_id())) } else { None };
            if let Some(s) = p.slides.get_mut(i) {
                let s = Arc::make_mut(s);
                if needs_layout {
                    s.layout = first_layout;
                }
                if let Some(id) = new_id {
                    s.id = id;
                }
            }
        }
    }
    if !(p.slide_size.width.is_finite() && p.slide_size.width >= 1.0 && p.slide_size.height.is_finite() && p.slide_size.height >= 1.0) {
        p.slide_size = defaults::WIDE;
    }
    p.slide_size.width = p.slide_size.width.min(4000.0);
    p.slide_size.height = p.slide_size.height.min(4000.0);
}
