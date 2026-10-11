//! Matroska track metadata, codec configuration and content encodings.

use crate::codec::map_codec;
use crate::ebml::{Children, float, string, uint};
use crate::ids::*;
use crate::track::{AudioInfo, Colour, ContentEncoding, MasteringMetadata, Projection, Track, TrackKind, VideoInfo, gcd};

pub(super) fn parse_colour(d: &[u8]) -> Colour {
    let mut c = Colour::default();
    for e in Children::new(d) {
        let v = || Some(uint(e.data));
        match e.id {
            MATRIX_COEFFICIENTS => c.matrix_coefficients = v(),
            BITS_PER_CHANNEL => c.bits_per_channel = v(),
            CHROMA_SUBSAMPLING_HORZ => c.chroma_subsampling_horz = v(),
            CHROMA_SUBSAMPLING_VERT => c.chroma_subsampling_vert = v(),
            CB_SUBSAMPLING_HORZ => c.cb_subsampling_horz = v(),
            CB_SUBSAMPLING_VERT => c.cb_subsampling_vert = v(),
            CHROMA_SITING_HORZ => c.chroma_siting_horz = v(),
            CHROMA_SITING_VERT => c.chroma_siting_vert = v(),
            RANGE => c.range = v(),
            TRANSFER_CHARACTERISTICS => c.transfer_characteristics = v(),
            PRIMARIES => c.primaries = v(),
            MAX_CLL => c.max_cll = v(),
            MAX_FALL => c.max_fall = v(),
            MASTERING_METADATA => {
                let mut m = MasteringMetadata::default();
                for x in Children::new(e.data) {
                    let f = float(x.data);
                    match x.id {
                        PRIMARY_R_X => m.primaries[0].0 = f,
                        PRIMARY_R_Y => m.primaries[0].1 = f,
                        PRIMARY_G_X => m.primaries[1].0 = f,
                        PRIMARY_G_Y => m.primaries[1].1 = f,
                        PRIMARY_B_X => m.primaries[2].0 = f,
                        PRIMARY_B_Y => m.primaries[2].1 = f,
                        WHITE_POINT_X => m.white_point.0 = f,
                        WHITE_POINT_Y => m.white_point.1 = f,
                        LUMINANCE_MAX => m.luminance_max = f,
                        LUMINANCE_MIN => m.luminance_min = f,
                        _ => {}
                    }
                }
                c.mastering = Some(m);
            }
            _ => {}
        }
    }
    c
}

pub(super) fn parse_video(d: &[u8]) -> VideoInfo {
    let mut v = VideoInfo::default();
    for e in Children::new(d) {
        let u = uint(e.data);
        match e.id {
            PIXEL_WIDTH => v.pixel_width = u as u32,
            PIXEL_HEIGHT => v.pixel_height = u as u32,
            DISPLAY_WIDTH => v.display_width = Some(u as u32),
            DISPLAY_HEIGHT => v.display_height = Some(u as u32),
            DISPLAY_UNIT => v.display_unit = u,
            PIXEL_CROP_TOP => v.crop.0 = u as u32,
            PIXEL_CROP_BOTTOM => v.crop.1 = u as u32,
            PIXEL_CROP_LEFT => v.crop.2 = u as u32,
            PIXEL_CROP_RIGHT => v.crop.3 = u as u32,
            FLAG_INTERLACED => v.flag_interlaced = u,
            FIELD_ORDER => v.field_order = Some(u),
            STEREO_MODE => v.stereo_mode = Some(u),
            ALPHA_MODE => v.alpha_mode = u,
            COLOUR_SPACE => v.colour_space = e.data.try_into().ok(),
            COLOUR => v.colour = Some(parse_colour(e.data)),
            PROJECTION => {
                let mut p = Projection::default();
                for x in Children::new(e.data) {
                    match x.id {
                        PROJECTION_TYPE => p.projection_type = uint(x.data),
                        PROJECTION_POSE_YAW => p.yaw = float(x.data),
                        PROJECTION_POSE_PITCH => p.pitch = float(x.data),
                        PROJECTION_POSE_ROLL => p.roll = float(x.data),
                        _ => {}
                    }
                }
                v.projection = Some(p);
            }
            _ => {}
        }
    }
    v
}

pub(super) fn parse_audio(d: &[u8]) -> AudioInfo {
    let mut a = AudioInfo::default();
    for e in Children::new(d) {
        match e.id {
            SAMPLING_FREQUENCY => a.sampling_frequency = float(e.data),
            OUTPUT_SAMPLING_FREQUENCY => a.output_sampling_frequency = Some(float(e.data)),
            CHANNELS => a.channels = uint(e.data),
            BIT_DEPTH => a.bit_depth = Some(uint(e.data)),
            _ => {}
        }
    }
    a
}

pub(super) fn parse_encodings(d: &[u8]) -> Vec<ContentEncoding> {
    let mut list: Vec<(u64, ContentEncoding)> = Vec::new();
    for ce in Children::new(d).filter(|e| e.id == CONTENT_ENCODING) {
        let (mut order, mut scope, mut ty) = (0u64, 1u64, 0u64);
        let mut comp: Option<(u64, Vec<u8>)> = None;
        for e in Children::new(ce.data) {
            match e.id {
                CONTENT_ENCODING_ORDER => order = uint(e.data),
                CONTENT_ENCODING_SCOPE => scope = uint(e.data),
                CONTENT_ENCODING_TYPE => ty = uint(e.data),
                CONTENT_COMPRESSION => {
                    let (mut algo, mut settings) = (0u64, Vec::new());
                    for x in Children::new(e.data) {
                        match x.id {
                            CONTENT_COMP_ALGO => algo = uint(x.data),
                            CONTENT_COMP_SETTINGS => settings = x.data.to_vec(),
                            _ => {}
                        }
                    }
                    comp = Some((algo, settings));
                }
                _ => {}
            }
        }
        let enc = if ty == 1 {
            ContentEncoding::Encryption { scope }
        } else {
            let (algo, settings) = comp.unwrap_or((0, Vec::new()));
            if algo == 3 {
                ContentEncoding::HeaderStripping { bytes: settings, scope }
            } else {
                ContentEncoding::Compression { algo, settings, scope }
            }
        };
        list.push((order, enc));
    }
    // highest order is applied last on muxing → first to undo; keep decode order
    list.sort_by_key(|e| std::cmp::Reverse(e.0));
    list.into_iter().map(|e| e.1).collect()
}

pub(crate) fn parse_tracks(d: &[u8], timestamp_scale: u64) -> Vec<Track> {
    let g = gcd(timestamp_scale, 1_000_000_000);
    let timebase = (timestamp_scale / g, 1_000_000_000 / g);
    let mut out = Vec::new();
    for te in Children::new(d).filter(|e| e.id == TRACK_ENTRY) {
        let mut t = Track {
            number: 0,
            uid: 0,
            kind: TrackKind::Other(0),
            enabled: true,
            default: true,
            forced: false,
            hearing_impaired: false,
            lacing: true,
            default_duration_ns: None,
            name: String::new(),
            language: "eng".into(),
            language_bcp47: None,
            codec_id: String::new(),
            codec_private: Vec::new(),
            codec_name: String::new(),
            codec: crate::Codec::Other(String::new()),
            codec_delay_ns: 0,
            seek_pre_roll_ns: 0,
            video: None,
            audio: None,
            encodings: Vec::new(),
            timebase,
            samples: Vec::new(),
            sync: Vec::new(),
        };
        for e in Children::new(te.data) {
            match e.id {
                TRACK_NUMBER => t.number = uint(e.data),
                TRACK_UID => t.uid = uint(e.data),
                TRACK_TYPE => t.kind = TrackKind::from_type(uint(e.data)),
                FLAG_ENABLED => t.enabled = uint(e.data) != 0,
                FLAG_DEFAULT => t.default = uint(e.data) != 0,
                FLAG_FORCED => t.forced = uint(e.data) != 0,
                FLAG_HEARING_IMPAIRED => t.hearing_impaired = uint(e.data) != 0,
                FLAG_LACING => t.lacing = uint(e.data) != 0,
                DEFAULT_DURATION => t.default_duration_ns = Some(uint(e.data)).filter(|&v| v > 0),
                NAME => t.name = string(e.data),
                LANGUAGE => t.language = string(e.data),
                LANGUAGE_BCP47 => t.language_bcp47 = Some(string(e.data)),
                CODEC_ID => t.codec_id = string(e.data),
                CODEC_PRIVATE => t.codec_private = e.data.to_vec(),
                CODEC_NAME => t.codec_name = string(e.data),
                CODEC_DELAY => t.codec_delay_ns = uint(e.data),
                SEEK_PRE_ROLL => t.seek_pre_roll_ns = uint(e.data),
                VIDEO => t.video = Some(parse_video(e.data)),
                AUDIO => t.audio = Some(parse_audio(e.data)),
                CONTENT_ENCODINGS => t.encodings = parse_encodings(e.data),
                _ => {}
            }
        }
        if t.number == 0 {
            continue;
        }
        if t.kind == TrackKind::Audio && t.audio.is_none() {
            t.audio = Some(AudioInfo::default());
        }
        // CodecPrivate may itself be header-stripped (scope bit 2 = private data)
        for enc in &t.encodings {
            if let ContentEncoding::HeaderStripping { bytes, scope } = enc
                && scope & 2 != 0
            {
                let mut p = bytes.clone();
                p.extend_from_slice(&t.codec_private);
                t.codec_private = p;
            }
        }
        t.codec = map_codec(&t.codec_id, &t.codec_private, t.audio.as_ref());
        out.push(t);
    }
    out
}
