//! The native `.deckcraft` format: a zip holding `presentation.json` (the model, minus media
//! bytes), `media/<id>` files with the embedded media, and `mimetype`.
//! The manifest must identify the current DeckCraft format version.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod archive;
pub use archive::{load, save, sniff};

mod repair;
pub use repair::repair;

mod outline;
pub use outline::{outline_to_slides, slides_to_outline};

pub const MIMETYPE: &str = "application/vnd.storyteller.deckcraft+zip";
pub const EXTENSION: &str = "deckcraft";
/// Format version written into the manifest.
pub const VERSION: u32 = 1;
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("not a DeckCraft file: {0}")]
    NotOurs(String),
    #[error("damaged file: {0}")]
    Damaged(String),
    #[error("write failed: {0}")]
    Write(String),
}

pub type Result<T> = std::result::Result<T, FormatError>;

#[cfg(test)]
mod tests {
    use super::*;
    use deckcraft_model::{Presentation, defaults};
    use std::sync::Arc;

    #[test]
    fn roundtrip_with_media() {
        let mut p = Presentation::default();
        let id = p.add_media("a.png", "image/png", vec![1, 2, 3, 4]);
        let bytes = save(&p).unwrap();
        assert!(sniff(&bytes));
        let q = load(&bytes).unwrap();
        assert_eq!(q.slides.len(), p.slides.len());
        assert_eq!(q.media(id).map(|m| m.data.as_slice()), Some(&[1u8, 2, 3, 4][..]));
        assert_eq!(serde_json::to_value(&q.slides).unwrap(), serde_json::to_value(&p.slides).unwrap());
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(load(b"").is_err());
        assert!(load(b"PK\x03\x04garbage").is_err());
        assert!(load(&[0u8; 1000]).is_err());
        let mut b = save(&Presentation::default()).unwrap();
        let n = b.len();
        b.truncate(n / 2);
        assert!(load(&b).is_err());
    }

    #[test]
    fn repair_fixes_missing_layout_and_size() {
        let mut p = Presentation::default();
        Arc::make_mut(&mut p.slides[0]).layout = deckcraft_model::LayoutId(99999);
        p.slide_size.width = f64::NAN;
        repair(&mut p);
        assert!(p.validate().is_empty());
        assert_eq!(p.slide_size.width, 960.0);
    }

    #[test]
    fn outline_roundtrip() {
        let mut p = defaults::blank_presentation(defaults::WIDE, Default::default(), false);
        let n = outline_to_slides(&mut p, "Intro\n\tPoint one\n\t\tSub point\nSecond\n\tMore");
        assert_eq!(n, 2);
        let o = slides_to_outline(&p);
        assert!(o.contains("Intro\n\tPoint one\n\t\tSub point\nSecond"), "{o}");
    }
}
