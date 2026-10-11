//! Segment-level metadata: Info, SeekHead, Cues, Chapters, Tags, Attachments — types and parsers
//! for their in-memory element data.

mod tracks;
pub(crate) use tracks::parse_tracks;

mod tags;
pub(crate) use tags::parse_tags;
pub use tags::{SimpleTag, Tag, TagTargets};

mod chapters;
pub(crate) use chapters::parse_chapters;
pub use chapters::{Chapter, ChapterDisplay, Edition};

use crate::ebml::{Children, float, int, string, uint};
use crate::ids::*;

/// `Info` element.
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentInfo {
    /// Nanoseconds per timestamp tick (default 1 000 000 = 1 ms).
    pub timestamp_scale: u64,
    /// `Duration` in timestamp ticks (float), if present.
    pub duration: Option<f64>,
    pub title: Option<String>,
    pub muxing_app: String,
    pub writing_app: String,
    /// `DateUTC`: nanoseconds since 2001-01-01T00:00:00 UTC.
    pub date_utc: Option<i64>,
    pub segment_uuid: Option<[u8; 16]>,
}

impl Default for SegmentInfo {
    fn default() -> Self {
        Self {
            timestamp_scale: 1_000_000,
            duration: None,
            title: None,
            muxing_app: String::new(),
            writing_app: String::new(),
            date_utc: None,
            segment_uuid: None,
        }
    }
}

/// One `SeekHead` entry: level-1 element ID and its position relative to the Segment data start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeekEntry {
    pub id: u32,
    pub position: u64,
}

/// One `CueTrackPositions` of a `CuePoint`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CuePosition {
    pub track: u64,
    /// Cluster position relative to the Segment data start.
    pub cluster_position: u64,
    /// Block position relative to the Cluster data start.
    pub relative_position: Option<u64>,
    pub block_number: Option<u64>,
    pub duration: Option<u64>,
}

/// One `CuePoint`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CuePoint {
    /// In timestamp ticks.
    pub time: u64,
    pub positions: Vec<CuePosition>,
}

/// A cluster found while scanning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClusterInfo {
    /// Absolute offset of the Cluster element header.
    pub offset: u64,
    /// Absolute offset of the first child.
    pub data_start: u64,
    /// Absolute end offset (for unknown-size clusters, where the next top-level element starts).
    pub end: u64,
    /// Cluster `Timestamp` in ticks.
    pub timestamp: u64,
}

/// `AttachedFile` (listed only; the payload is located by `data_offset`/`data_size`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attachment {
    pub uid: u64,
    pub name: String,
    pub media_type: String,
    pub description: Option<String>,
    pub data_offset: u64,
    pub data_size: u64,
}

pub(crate) fn parse_info(d: &[u8]) -> SegmentInfo {
    let mut info = SegmentInfo::default();
    for e in Children::new(d) {
        match e.id {
            TIMESTAMP_SCALE => {
                let v = uint(e.data);
                if v > 0 {
                    info.timestamp_scale = v;
                }
            }
            DURATION => info.duration = Some(float(e.data)).filter(|v| v.is_finite() && *v >= 0.0),
            TITLE => info.title = Some(string(e.data)),
            MUXING_APP => info.muxing_app = string(e.data),
            WRITING_APP => info.writing_app = string(e.data),
            DATE_UTC => info.date_utc = Some(int(e.data)),
            SEGMENT_UUID => info.segment_uuid = e.data.try_into().ok(),
            _ => {}
        }
    }
    info
}

pub(crate) fn parse_seekhead(d: &[u8]) -> Vec<SeekEntry> {
    let mut out = Vec::new();
    for s in Children::new(d).filter(|e| e.id == SEEK) {
        let (mut id, mut pos) = (None, None);
        for e in Children::new(s.data) {
            match e.id {
                SEEK_ID => id = Some(uint(e.data) as u32),
                SEEK_POSITION => pos = Some(uint(e.data)),
                _ => {}
            }
        }
        if let (Some(id), Some(position)) = (id, pos) {
            out.push(SeekEntry { id, position });
        }
    }
    out
}

pub(crate) fn parse_cues(d: &[u8], out: &mut Vec<CuePoint>) {
    for cp in Children::new(d).filter(|e| e.id == CUE_POINT) {
        let mut time = None;
        let mut positions = Vec::new();
        for e in Children::new(cp.data) {
            match e.id {
                CUE_TIME => time = Some(uint(e.data)),
                CUE_TRACK_POSITIONS => {
                    let mut p = CuePosition { track: 0, cluster_position: u64::MAX, relative_position: None, block_number: None, duration: None };
                    for c in Children::new(e.data) {
                        match c.id {
                            CUE_TRACK => p.track = uint(c.data),
                            CUE_CLUSTER_POSITION => p.cluster_position = uint(c.data),
                            CUE_RELATIVE_POSITION => p.relative_position = Some(uint(c.data)),
                            CUE_BLOCK_NUMBER => p.block_number = Some(uint(c.data)),
                            CUE_DURATION => p.duration = Some(uint(c.data)),
                            _ => {}
                        }
                    }
                    if p.track != 0 && p.cluster_position != u64::MAX {
                        positions.push(p);
                    }
                }
                _ => {}
            }
        }
        if let Some(time) = time
            && !positions.is_empty()
        {
            out.push(CuePoint { time, positions });
        }
    }
}
