//! Video track discovery, sample metadata and demux access.

use super::codecs::{Av1, H264, Hevc, Vp9};
use super::{Codec, dec_err};
use crate::{Container, MediaError, Result};
use deckcraft_isobmff::CodecConfig;
use deckcraft_matroska::Codec as MkvCodec;

/// The video track's samples in decode order: (pts in ticks, sync).
pub(super) struct Track {
    pub(super) samples: Vec<(i64, bool)>,
    /// Seconds per tick (num, den).
    pub(super) timebase: (i64, i64),
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) codec: String,
}

pub(super) enum Demux {
    Mp4(Box<deckcraft_isobmff::Mp4File>, usize),
    Mkv(Box<deckcraft_matroska::MkvFile>, usize),
}

impl Demux {
    pub(super) fn read(&self, bytes: &[u8], i: usize) -> Result<Vec<u8>> {
        match self {
            Demux::Mp4(f, t) => f.read_sample(bytes, *t, i).map_err(dec_err),
            Demux::Mkv(f, t) => f.read_sample(bytes, *t, i).map_err(dec_err),
        }
    }
}

pub(super) fn open_track(bytes: &[u8]) -> Result<(Demux, Track, Box<dyn Codec>)> {
    match Container::sniff(bytes) {
        Container::Mp4 => {
            let f = deckcraft_isobmff::open(bytes).map_err(dec_err)?;
            let ti = f.track_of_kind(deckcraft_isobmff::TrackKind::Video).ok_or(MediaError::Missing("video track"))?;
            let t = &f.tracks[ti];
            let entry = t.entries.first().ok_or(MediaError::Missing("video track"))?;
            let codec: Box<dyn Codec> = match &entry.codec {
                CodecConfig::Avc(a) => Box::new(H264::new(a.to_bytes())?),
                CodecConfig::Hevc(c) => Box::new(Hevc::new(c.to_bytes())?),
                CodecConfig::Vp9(_) => Box::new(Vp9 { dec: deckcraft_vp9::Decoder::new() }),
                CodecConfig::Av1(c) => Box::new(Av1 { dec: deckcraft_av1::Decoder::new(), config_obus: c.config_obus.clone(), primed: false }),
                other => return Err(MediaError::Unsupported(format!("{} video", codec_name(other)))),
            };
            let (w, h) = entry.video.as_ref().map(|v| (v.width as u32, v.height as u32)).unwrap_or((t.width, t.height));
            let samples = (0..t.samples.len()).map(|i| (t.presentation_pts(i).unwrap_or(0), t.samples[i].is_sync)).collect();
            let track = Track { samples, timebase: (1, t.timescale.max(1) as i64), width: w, height: h, codec: codec_name(&entry.codec) };
            Ok((Demux::Mp4(Box::new(f), ti), track, codec))
        }
        Container::Matroska => {
            let f = deckcraft_matroska::open(bytes).map_err(dec_err)?;
            let ti = f
                .tracks
                .iter()
                .position(|t| t.kind == deckcraft_matroska::TrackKind::Video && !t.samples.is_empty())
                .ok_or(MediaError::Missing("video track"))?;
            let t = &f.tracks[ti];
            let codec: Box<dyn Codec> = match &t.codec {
                MkvCodec::Avc { avcc } => Box::new(H264::new(avcc.clone())?),
                MkvCodec::Hevc { hvcc } => Box::new(Hevc::new(hvcc.clone())?),
                MkvCodec::Vp9 { .. } => Box::new(Vp9 { dec: deckcraft_vp9::Decoder::new() }),
                MkvCodec::Av1 { av1c } => Box::new(Av1 {
                    dec: deckcraft_av1::Decoder::new(),
                    config_obus: deckcraft_isobmff::Av1Config::parse(av1c).map(|c| c.config_obus).unwrap_or_default(),
                    primed: false,
                }),
                other => return Err(MediaError::Unsupported(format!("{} video", mkv_codec_name(other)))),
            };
            let (w, h) = t.video.as_ref().map(|v| (v.pixel_width, v.pixel_height)).unwrap_or((0, 0));
            let samples = t.samples.iter().map(|s| (s.pts, s.keyframe)).collect();
            let track = Track {
                samples,
                timebase: (t.timebase.0.max(1) as i64, t.timebase.1.max(1) as i64),
                width: w,
                height: h,
                codec: mkv_codec_name(&t.codec),
            };
            Ok((Demux::Mkv(Box::new(f), ti), track, codec))
        }
        Container::Asf => Err(MediaError::Unsupported("Windows Media Video (WMV)".into())),
        _ => Err(MediaError::Missing("video track")),
    }
}

pub(crate) fn codec_name(c: &CodecConfig) -> String {
    match c {
        CodecConfig::Avc(_) => "H.264".into(),
        CodecConfig::Hevc(_) => "HEVC".into(),
        CodecConfig::Av1(_) => "AV1".into(),
        CodecConfig::Vp9(_) => "VP9".into(),
        CodecConfig::ProRes { .. } => "Apple ProRes".into(),
        CodecConfig::Jpeg { .. } => "Motion JPEG".into(),
        CodecConfig::Dnx { .. } => "DNxHD".into(),
        CodecConfig::Aac(_) => "AAC".into(),
        CodecConfig::Mp3 => "MP3".into(),
        CodecConfig::Pcm(_) => "PCM".into(),
        CodecConfig::Alac { .. } => "ALAC".into(),
        CodecConfig::Opus(_) => "Opus".into(),
        CodecConfig::Flac(_) => "FLAC".into(),
        CodecConfig::Ac3 { .. } => "AC-3".into(),
        CodecConfig::Eac3 { .. } => "E-AC-3".into(),
        CodecConfig::Timecode(_) => "timecode".into(),
        CodecConfig::Unknown { fourcc, .. } => format!("{fourcc:?}"),
    }
}

pub(crate) fn mkv_codec_name(c: &MkvCodec) -> String {
    match c {
        MkvCodec::Avc { .. } => "H.264".into(),
        MkvCodec::Hevc { .. } => "HEVC".into(),
        MkvCodec::Vp8 => "VP8".into(),
        MkvCodec::Vp9 { .. } => "VP9".into(),
        MkvCodec::Av1 { .. } => "AV1".into(),
        MkvCodec::Aac { .. } => "AAC".into(),
        MkvCodec::Opus { .. } => "Opus".into(),
        MkvCodec::Vorbis { .. } => "Vorbis".into(),
        MkvCodec::Flac { .. } => "FLAC".into(),
        MkvCodec::Mp3 => "MP3".into(),
        MkvCodec::Pcm { .. } => "PCM".into(),
        other => other.name(),
    }
}
