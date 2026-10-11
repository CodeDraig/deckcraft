//! Video decoding: MP4/MOV and WebM/Matroska demuxing, H.264 / HEVC / VP9 / AV1 decoding (our own
//! decoders), RGBA frames and poster frames.

mod track;
use track::{Demux, Track, open_track};
pub(crate) use track::{codec_name, mkv_codec_name};

mod codecs;

use std::collections::VecDeque;

use crate::{Bytes, MediaError, Result};

/// A decoded picture, RGBA8 (straight alpha, always opaque), with its presentation time.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Presentation time in seconds from the start of the media.
    pub time: f64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A picture from a codec, before colour conversion; `pts` in track ticks.
struct Pic {
    pts: i64,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

trait Codec: Send {
    fn decode(&mut self, sample: &[u8], pts: i64) -> Result<Vec<Pic>>;
    fn flush(&mut self) -> Vec<Pic>;
    fn reset(&mut self);
}

fn dec_err(e: impl std::fmt::Display) -> MediaError {
    MediaError::Corrupt(e.to_string())
}

// ---------- codecs ----------

// ---------- containers ----------

/// Decodes the video track of a file, frame by frame in presentation order.
pub struct VideoDecoder {
    bytes: Bytes,
    demux: Demux,
    track: Track,
    codec: Box<dyn Codec>,
    /// Next sample to feed (decode order).
    next: usize,
    /// Decoded pictures waiting, in presentation order.
    ready: VecDeque<Pic>,
    /// Frames before this time (s) are dropped (after a seek).
    skip_before: f64,
    eos: bool,
    errors: u32,
}

impl VideoDecoder {
    pub fn open(bytes: Bytes) -> Result<VideoDecoder> {
        let (demux, track, codec) = open_track(&bytes)?;
        Ok(VideoDecoder { bytes, demux, track, codec, next: 0, ready: VecDeque::new(), skip_before: f64::NEG_INFINITY, eos: false, errors: 0 })
    }
    pub fn size(&self) -> (u32, u32) {
        (self.track.width, self.track.height)
    }
    pub fn codec(&self) -> &str {
        &self.track.codec
    }
    pub fn frame_count(&self) -> usize {
        self.track.samples.len()
    }
    fn secs(&self, pts: i64) -> f64 {
        pts as f64 * self.track.timebase.0 as f64 / self.track.timebase.1 as f64
    }

    /// The next frame in presentation order, `None` at the end.
    pub fn next_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            // A picture is safe to emit once the decoder has returned it (they come out in
            // presentation order); keep the queue sorted for decoders that emit in batches.
            if let Some(p) = self.ready.pop_front() {
                let time = self.secs(p.pts);
                if time + 1e-6 < self.skip_before {
                    continue;
                }
                return Ok(Some(Frame { time, width: p.width, height: p.height, rgba: p.rgba }));
            }
            if self.eos {
                return Ok(None);
            }
            if self.next >= self.track.samples.len() {
                self.eos = true;
                let mut pics = self.codec.flush();
                pics.sort_by_key(|p| p.pts);
                self.ready.extend(pics);
                continue;
            }
            let i = self.next;
            self.next += 1;
            let pts = self.track.samples[i].0;
            let data = self.demux.read(&self.bytes, i)?;
            match self.codec.decode(&data, pts) {
                Ok(mut pics) => {
                    pics.sort_by_key(|p| p.pts);
                    self.ready.extend(pics);
                }
                Err(e) => {
                    // A corrupt sample: carry on (the picture is lost), give up after many.
                    self.errors += 1;
                    log::debug!("video sample {i}: {e}");
                    if self.errors > 50 {
                        return Err(e);
                    }
                }
            }
        }
    }

    /// Position so that the next frame is the one shown at `t` seconds (decoding resumes at the
    /// preceding sync sample).
    pub fn seek(&mut self, t: f64) {
        let target = (t * self.track.timebase.1 as f64 / self.track.timebase.0 as f64) as i64;
        // The last sync sample whose pts is at or before the target.
        let mut start = 0;
        for (i, (pts, sync)) in self.track.samples.iter().enumerate() {
            if *sync && *pts <= target {
                start = i;
            }
        }
        self.codec.reset();
        self.ready.clear();
        self.next = start;
        self.eos = false;
        // Keep the frame on screen at `t`: the one whose pts is the last ≤ t.
        let shown = self.track.samples.iter().map(|s| s.0).filter(|p| *p <= target).max();
        self.skip_before = shown.map(|p| self.secs(p)).unwrap_or(f64::NEG_INFINITY);
    }
}

/// The frame shown at `t` seconds (the first frame when `t` is 0).
pub fn poster_frame(bytes: &Bytes, t: f64) -> Result<Frame> {
    let mut d = VideoDecoder::open(bytes.clone())?;
    if t > 0.0 {
        d.seek(t);
    }
    let mut last = None;
    while let Some(f) = d.next_frame()? {
        if f.time > t + 1e-6 && last.is_some() {
            break;
        }
        let done = f.time + 1e-6 >= t;
        last = Some(f);
        if done {
            break;
        }
    }
    last.ok_or(MediaError::Corrupt("no picture could be decoded".into()))
}

/// [`poster_frame`] as PNG bytes.
pub fn poster_png(bytes: &Bytes, t: f64) -> Result<Vec<u8>> {
    let f = poster_frame(bytes, t)?;
    let img = image::RgbaImage::from_raw(f.width, f.height, f.rgba).ok_or(MediaError::Corrupt("bad picture size".into()))?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(dec_err)?;
    Ok(out)
}
