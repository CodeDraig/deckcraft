//! NAL framing and emulation prevention.

use super::{BitError, Result};

/// Remove emulation-prevention bytes (`00 00 03` → `00 00`) from a NAL payload.
pub fn unescape_rbsp(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut zeros = 0;
    for &b in data {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            continue;
        }
        out.push(b);
        zeros = if b == 0 { zeros + 1 } else { 0 };
    }
    out
}

/// Insert emulation-prevention bytes so no `00 00 0x` (x ≤ 3) sequence appears.
pub fn escape_rbsp(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 64);
    let mut zeros = 0;
    for &b in data {
        if zeros >= 2 && b <= 3 {
            out.push(3);
            zeros = 0;
        }
        out.push(b);
        zeros = if b == 0 { zeros + 1 } else { 0 };
    }
    out
}

/// Split an Annex-B byte stream (start codes `00 00 01` / `00 00 00 01`) into NAL units (escaped payloads).
pub fn annexb_nals(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (k, &s) in starts.iter().enumerate() {
        let mut e = if k + 1 < starts.len() { starts[k + 1] - 3 } else { data.len() };
        // trim trailing zero bytes (part of a 4-byte start code or trailing_zero_8bits)
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        if e > s {
            out.push(&data[s..e]);
        }
    }
    out
}

/// Split a length-prefixed sample (AVCC/HVCC, `length_size` 1/2/4 bytes) into NAL units.
pub fn length_prefixed_nals(data: &[u8], length_size: usize) -> Result<Vec<&[u8]>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if i + length_size > data.len() {
            return Err(BitError::Eof);
        }
        let mut len = 0usize;
        for k in 0..length_size {
            len = (len << 8) | data[i + k] as usize;
        }
        i += length_size;
        if i + len > data.len() {
            return Err(BitError::Eof);
        }
        out.push(&data[i..i + len]);
        i += len;
    }
    Ok(out)
}
