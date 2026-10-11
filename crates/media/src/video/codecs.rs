//! Video codec instances and decoded-picture conversion.

use super::{Codec, Pic, dec_err};
use crate::Result;
use crate::yuv::{self, Matrix, Planes};

pub(super) struct H264 {
    avcc: Vec<u8>,
    dec: deckcraft_h264::Decoder,
}

impl H264 {
    pub(super) fn new(avcc: Vec<u8>) -> Result<H264> {
        let dec = deckcraft_h264::Decoder::from_avcc(&avcc).map_err(dec_err)?;
        Ok(H264 { avcc, dec })
    }
    fn convert(p: deckcraft_h264::Picture) -> Pic {
        let (w, h) = (p.width as usize, p.height as usize);
        let planes = Planes {
            y: &p.y,
            u: &p.u,
            v: &p.v,
            y_stride: p.y_stride,
            c_stride: p.uv_stride,
            width: w,
            height: h,
            sx: u32::from(p.chroma_width < p.width),
            sy: u32::from(p.chroma_height < p.height),
            bits: 8,
            mono: p.chroma_width == 0,
        };
        let rgba = yuv::to_rgba(&planes, Matrix::from_code(p.color.matrix, p.height), p.color.full_range);
        Pic { pts: p.pts, width: p.width, height: p.height, rgba }
    }
}

impl Codec for H264 {
    fn decode(&mut self, sample: &[u8], pts: i64) -> Result<Vec<Pic>> {
        Ok(self.dec.decode(sample, pts).map_err(dec_err)?.into_iter().map(H264::convert).collect())
    }
    fn flush(&mut self) -> Vec<Pic> {
        self.dec.flush().into_iter().map(H264::convert).collect()
    }
    fn reset(&mut self) {
        if let Ok(d) = deckcraft_h264::Decoder::from_avcc(&self.avcc) {
            self.dec = d;
        }
    }
}

/// Convert an HEVC / VP9 style picture (planes of u8 or u16 with strides).
macro_rules! plane_pic {
    ($p:expr, $Plane:path, $sx:expr, $sy:expr, $matrix:expr, $full:expr, $mono:expr) => {{
        let p = $p;
        use $Plane as P;
        let (w, h) = (p.width as usize, p.height as usize);
        let rgba = match (&p.y, &p.u, &p.v) {
            (P::U8(y), P::U8(u), P::U8(v)) => yuv::to_rgba(
                &Planes { y, u, v, y_stride: p.y_stride, c_stride: p.uv_stride, width: w, height: h, sx: $sx, sy: $sy, bits: 8, mono: $mono },
                $matrix,
                $full,
            ),
            (P::U16(y), P::U16(u), P::U16(v)) => yuv::to_rgba(
                &Planes {
                    y,
                    u,
                    v,
                    y_stride: p.y_stride,
                    c_stride: p.uv_stride,
                    width: w,
                    height: h,
                    sx: $sx,
                    sy: $sy,
                    bits: p.bit_depth as u32,
                    mono: $mono,
                },
                $matrix,
                $full,
            ),
            _ => vec![0; w * h * 4],
        };
        Pic { pts: p.pts, width: p.width, height: p.height, rgba }
    }};
}

pub(super) struct Hevc {
    hvcc: Vec<u8>,
    dec: deckcraft_hevc::Decoder,
}

impl Hevc {
    pub(super) fn new(hvcc: Vec<u8>) -> Result<Hevc> {
        let dec = deckcraft_hevc::Decoder::from_hvcc(&hvcc).map_err(dec_err)?;
        Ok(Hevc { hvcc, dec })
    }
    fn convert(p: deckcraft_hevc::Picture) -> Pic {
        let sx = u32::from(p.chroma_width < p.width);
        let sy = u32::from(p.chroma_height < p.height);
        let m = Matrix::from_code(p.color.matrix, p.height);
        plane_pic!(&p, deckcraft_hevc::Plane, sx, sy, m, p.color.full_range, p.chroma_width == 0)
    }
}

impl Codec for Hevc {
    fn decode(&mut self, sample: &[u8], pts: i64) -> Result<Vec<Pic>> {
        Ok(self.dec.decode(sample, pts).map_err(dec_err)?.into_iter().map(Hevc::convert).collect())
    }
    fn flush(&mut self) -> Vec<Pic> {
        self.dec.flush().into_iter().map(Hevc::convert).collect()
    }
    fn reset(&mut self) {
        if let Ok(d) = deckcraft_hevc::Decoder::from_hvcc(&self.hvcc) {
            self.dec = d;
        }
    }
}

pub(super) struct Vp9 {
    pub(super) dec: deckcraft_vp9::Decoder,
}

impl Vp9 {
    fn convert(p: deckcraft_vp9::Picture) -> Pic {
        let (w, h) = (p.width as usize, p.height as usize);
        if p.color.color_space == 7 {
            // RGB (4:4:4): the planes carry G, B, R.
            let shift = p.bit_depth.saturating_sub(8);
            let at = |pl: &deckcraft_vp9::Plane, i: usize| if i < pl.len() { (pl.get(i) >> shift) as u8 } else { 0 };
            let mut rgba = Vec::with_capacity(w * h * 4);
            for row in 0..h {
                for col in 0..w {
                    let i = row * p.y_stride + col;
                    let c = row * p.uv_stride + col;
                    rgba.extend_from_slice(&[at(&p.v, c), at(&p.y, i), at(&p.u, c), 255]);
                }
            }
            return Pic { pts: p.pts, width: p.width, height: p.height, rgba };
        }
        // color_space (VP9 §7.2.2): 1 BT.601, 2 BT.709, 3 SMPTE-170, 4 SMPTE-240, 5 BT.2020.
        let m = match p.color.color_space {
            1 | 3 => Matrix::Bt601,
            2 | 4 => Matrix::Bt709,
            5 => Matrix::Bt2020,
            _ => Matrix::from_code(2, p.height),
        };
        plane_pic!(&p, deckcraft_vp9::Plane, u32::from(p.subsampling_x), u32::from(p.subsampling_y), m, p.color.full_range, false)
    }
}

impl Codec for Vp9 {
    fn decode(&mut self, sample: &[u8], pts: i64) -> Result<Vec<Pic>> {
        Ok(self.dec.decode(sample, pts).map_err(dec_err)?.into_iter().map(Vp9::convert).collect())
    }
    fn flush(&mut self) -> Vec<Pic> {
        self.dec.flush().into_iter().map(Vp9::convert).collect()
    }
    fn reset(&mut self) {
        self.dec.reset();
    }
}

pub(super) struct Av1 {
    pub(super) dec: deckcraft_av1::Decoder,
    pub(super) config_obus: Vec<u8>,
    pub(super) primed: bool,
}

impl Av1 {
    fn convert(p: deckcraft_av1::Picture) -> Pic {
        let (w, h) = (p.width as usize, p.height as usize);
        let cw = p.plane_width(1) as usize;
        let rgba = yuv::to_rgba(
            &Planes {
                y: &p.planes[0],
                u: &p.planes[1],
                v: &p.planes[2],
                y_stride: w,
                c_stride: cw,
                width: w,
                height: h,
                sx: p.subsampling_x as u32,
                sy: p.subsampling_y as u32,
                bits: p.bit_depth as u32,
                mono: p.mono_chrome,
            },
            Matrix::from_code(p.matrix_coefficients, p.height),
            p.full_range,
        );
        Pic { pts: p.pts, width: p.width, height: p.height, rgba }
    }
}

impl Codec for Av1 {
    fn decode(&mut self, sample: &[u8], pts: i64) -> Result<Vec<Pic>> {
        if !self.primed {
            self.primed = true;
            if !self.config_obus.is_empty() {
                self.dec.decode(&self.config_obus).map_err(dec_err)?;
            }
        }
        Ok(self.dec.decode_pts(sample, pts).map_err(dec_err)?.into_iter().map(Av1::convert).collect())
    }
    fn flush(&mut self) -> Vec<Pic> {
        self.dec.flush().into_iter().map(Av1::convert).collect()
    }
    fn reset(&mut self) {
        self.dec = deckcraft_av1::Decoder::new();
        self.primed = false;
    }
}
