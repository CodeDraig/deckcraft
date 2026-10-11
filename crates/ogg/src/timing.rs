//! Codec packet durations, stream timing and trimming.

use super::Stream;

/// Duration of an Opus packet in 48 kHz samples from its TOC (RFC 6716 §3.1); `None` if invalid.
pub fn opus_packet_samples(prefix: [u8; 2], size: u32) -> Option<u32> {
    if size == 0 {
        return None;
    }
    let toc = prefix[0];
    let config = toc >> 3;
    let frame = match config {
        0..=11 => [480, 960, 1920, 2880][(config & 3) as usize],
        12..=15 => [480, 960][(config & 1) as usize],
        _ => [120, 240, 480, 960][(config & 3) as usize],
    };
    let frames = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => {
            if size < 2 {
                return None;
            }
            (prefix[1] & 0x3F) as u32
        }
    };
    let n = frame * frames;
    (n > 0 && n <= 5760).then_some(n)
}

/// RFC 7845 §4 timing of an Opus stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpusTiming {
    pub pre_skip: u32,
    /// Start of each packet's decoded output in output samples (48 kHz): 0 is the first sample
    /// after the pre-skip; the pre-skip samples have negative positions.
    pub starts: Vec<i64>,
    /// Decoded samples of each packet.
    pub durations: Vec<u32>,
    /// Playable samples (end trimming applied).
    pub total: i64,
}

impl OpusTiming {
    /// Timing of the Opus stream `s` (`pre_skip` from its `OpusHead`).
    pub fn of(stream: &Stream, pre_skip: u32) -> OpusTiming {
        let durations: Vec<u32> = stream.packets.iter().map(|p| opus_packet_samples(p.prefix, p.size).unwrap_or(0)).collect();
        let n = durations.len();
        let mut ends: Vec<Option<i64>> = vec![None; n];
        let mut i = 0;
        let mut cursor: Option<i64> = None;
        let mut trimmed_end: Option<i64> = None;
        while i < n {
            // packets completing on the same page: i..j, the last carries the page granule
            let mut j = i;
            while j < n && stream.packets[j].granule.is_none() {
                j += 1;
            }
            if j == n {
                // trailing packets without a granule (truncated file): continue from the cursor
                let mut c = cursor.unwrap_or(0);
                for k in i..n {
                    c += durations[k] as i64;
                    ends[k] = Some(c);
                }
                break;
            }
            let Some(g) = stream.packets[j].granule else { break };
            let sum: i64 = durations[i..=j].iter().map(|&d| d as i64).sum();
            let forward = cursor.map(|c| c + sum);
            let end = match forward {
                // end trimming on the last page (RFC 7845 §4.5): keep the forward positions
                Some(f) if stream.packets[j].eos && g < f => {
                    trimmed_end = Some(g);
                    f
                }
                Some(f) if f == g => f,
                // first page, or a discontinuity: trust the granule
                _ => g,
            };
            let mut e = end;
            for k in (i..=j).rev() {
                ends[k] = Some(e);
                e -= durations[k] as i64;
            }
            cursor = Some(end);
            i = j + 1;
        }
        let raw_starts: Vec<i64> = ends.iter().zip(&durations).map(|(e, &d)| e.unwrap_or(0) - d as i64).collect();
        let base = raw_starts.first().copied().unwrap_or(0).max(0);
        let shift = base + pre_skip as i64;
        let starts: Vec<i64> = raw_starts.iter().map(|s| s - shift).collect();
        let end_all = trimmed_end.or(cursor).unwrap_or(0);
        let total = (end_all - shift).max(0);
        OpusTiming { pre_skip, starts, durations, total }
    }
}

/// Vorbis identification header fields (Vorbis I §4.2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VorbisInfo {
    pub channels: u8,
    pub sample_rate: u32,
    pub bitrate_nominal: i32,
}

impl VorbisInfo {
    pub fn parse(h: &[u8]) -> Option<VorbisInfo> {
        if h.len() < 30 || !h.starts_with(b"\x01vorbis") {
            return None;
        }
        Some(VorbisInfo {
            channels: h[11],
            sample_rate: u32::from_le_bytes([h[12], h[13], h[14], h[15]]),
            bitrate_nominal: i32::from_le_bytes([h[20], h[21], h[22], h[23]]),
        })
    }
}
