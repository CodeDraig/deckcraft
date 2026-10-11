//! Matroska chapter trees and edition parsing.

use crate::ebml::{Children, string, uint};
use crate::ids::*;

/// One `ChapterDisplay`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChapterDisplay {
    pub string: String,
    pub languages: Vec<String>,
    pub languages_bcp47: Vec<String>,
    pub countries: Vec<String>,
}

/// `ChapterAtom` (possibly nested).
#[derive(Clone, Debug, PartialEq)]
pub struct Chapter {
    pub uid: u64,
    pub string_uid: Option<String>,
    pub start_ns: u64,
    pub end_ns: Option<u64>,
    pub hidden: bool,
    pub enabled: bool,
    pub displays: Vec<ChapterDisplay>,
    pub children: Vec<Chapter>,
}

impl Chapter {
    /// First display string, if any.
    pub fn title(&self) -> Option<&str> {
        self.displays.first().map(|d| d.string.as_str())
    }
}

/// `EditionEntry`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edition {
    pub uid: Option<u64>,
    pub hidden: bool,
    pub default: bool,
    pub ordered: bool,
    pub chapters: Vec<Chapter>,
}

pub(super) fn parse_chapter(d: &[u8]) -> Chapter {
    let mut c =
        Chapter { uid: 0, string_uid: None, start_ns: 0, end_ns: None, hidden: false, enabled: true, displays: Vec::new(), children: Vec::new() };
    for e in Children::new(d) {
        match e.id {
            CHAPTER_UID => c.uid = uint(e.data),
            CHAPTER_STRING_UID => c.string_uid = Some(string(e.data)),
            CHAPTER_TIME_START => c.start_ns = uint(e.data),
            CHAPTER_TIME_END => c.end_ns = Some(uint(e.data)),
            CHAPTER_FLAG_HIDDEN => c.hidden = uint(e.data) != 0,
            CHAPTER_FLAG_ENABLED => c.enabled = uint(e.data) != 0,
            CHAPTER_DISPLAY => {
                let mut disp = ChapterDisplay::default();
                for x in Children::new(e.data) {
                    match x.id {
                        CHAP_STRING => disp.string = string(x.data),
                        CHAP_LANGUAGE => disp.languages.push(string(x.data)),
                        CHAP_LANGUAGE_BCP47 => disp.languages_bcp47.push(string(x.data)),
                        CHAP_COUNTRY => disp.countries.push(string(x.data)),
                        _ => {}
                    }
                }
                if disp.languages.is_empty() {
                    disp.languages.push("eng".into());
                }
                c.displays.push(disp);
            }
            CHAPTER_ATOM => c.children.push(parse_chapter(e.data)),
            _ => {}
        }
    }
    c
}

pub(crate) fn parse_chapters(d: &[u8], out: &mut Vec<Edition>) {
    for ed in Children::new(d).filter(|e| e.id == EDITION_ENTRY) {
        let mut edition = Edition::default();
        for e in Children::new(ed.data) {
            match e.id {
                EDITION_UID => edition.uid = Some(uint(e.data)),
                EDITION_FLAG_HIDDEN => edition.hidden = uint(e.data) != 0,
                EDITION_FLAG_DEFAULT => edition.default = uint(e.data) != 0,
                EDITION_FLAG_ORDERED => edition.ordered = uint(e.data) != 0,
                CHAPTER_ATOM => edition.chapters.push(parse_chapter(e.data)),
                _ => {}
            }
        }
        out.push(edition);
    }
}
