//! Matroska tag targets and recursive tag parsing.

use crate::ebml::{Children, string, uint};
use crate::ids::*;

/// `Targets` of a `Tag`. Empty UID lists mean "the whole segment".
#[derive(Clone, Debug, PartialEq)]
pub struct TagTargets {
    pub type_value: u64,
    pub type_name: Option<String>,
    pub track_uids: Vec<u64>,
    pub edition_uids: Vec<u64>,
    pub chapter_uids: Vec<u64>,
    pub attachment_uids: Vec<u64>,
}

impl Default for TagTargets {
    fn default() -> Self {
        Self {
            type_value: 50,
            type_name: None,
            track_uids: Vec::new(),
            edition_uids: Vec::new(),
            chapter_uids: Vec::new(),
            attachment_uids: Vec::new(),
        }
    }
}

/// `SimpleTag` (possibly nested).
#[derive(Clone, Debug, PartialEq)]
pub struct SimpleTag {
    pub name: String,
    pub language: String,
    pub default: bool,
    pub string: Option<String>,
    pub binary: Option<Vec<u8>>,
    pub children: Vec<SimpleTag>,
}

/// `Tag`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tag {
    pub targets: TagTargets,
    pub simple: Vec<SimpleTag>,
}

pub(super) fn parse_simple_tag(d: &[u8]) -> SimpleTag {
    let mut t = SimpleTag { name: String::new(), language: "und".into(), default: true, string: None, binary: None, children: Vec::new() };
    for e in Children::new(d) {
        match e.id {
            TAG_NAME => t.name = string(e.data),
            TAG_LANGUAGE => t.language = string(e.data),
            TAG_DEFAULT => t.default = uint(e.data) != 0,
            TAG_STRING => t.string = Some(string(e.data)),
            TAG_BINARY => t.binary = Some(e.data.to_vec()),
            SIMPLE_TAG => t.children.push(parse_simple_tag(e.data)),
            _ => {}
        }
    }
    t
}

pub(crate) fn parse_tags(d: &[u8], out: &mut Vec<Tag>) {
    for tg in Children::new(d).filter(|e| e.id == TAG) {
        let mut tag = Tag::default();
        for e in Children::new(tg.data) {
            match e.id {
                TARGETS => {
                    for x in Children::new(e.data) {
                        match x.id {
                            TARGET_TYPE_VALUE => tag.targets.type_value = uint(x.data),
                            TARGET_TYPE => tag.targets.type_name = Some(string(x.data)),
                            TAG_TRACK_UID => tag.targets.track_uids.push(uint(x.data)),
                            TAG_EDITION_UID => tag.targets.edition_uids.push(uint(x.data)),
                            TAG_CHAPTER_UID => tag.targets.chapter_uids.push(uint(x.data)),
                            TAG_ATTACHMENT_UID => tag.targets.attachment_uids.push(uint(x.data)),
                            _ => {}
                        }
                    }
                }
                SIMPLE_TAG => tag.simple.push(parse_simple_tag(e.data)),
                _ => {}
            }
        }
        out.push(tag);
    }
}
