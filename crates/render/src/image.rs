//! Rendered RGBA images and image encoding.

/// A rendered image: premultiplied RGBA8.
#[derive(Clone, Debug, Default)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Image {
    /// Straight (unpremultiplied) RGBA.
    pub fn to_straight(&self) -> Vec<u8> {
        let mut out = self.pixels.clone();
        for px in out.as_chunks_mut::<4>().0 {
            let a = px.get(3).copied().unwrap_or(255) as u32;
            if a != 0 && a != 255 {
                for c in px.iter_mut().take(3) {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        out
    }
    pub fn to_png(&self) -> Vec<u8> {
        let mut px = self.to_straight();
        px.resize(self.width as usize * self.height as usize * 4, 0);
        let Some(img) = image::RgbaImage::from_raw(self.width, self.height, px) else { return Vec::new() };
        let mut buf = Vec::new();
        if let Err(e) = img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png) {
            log::error!("PNG encode: {e}");
            return Vec::new();
        }
        buf
    }
    pub fn to_jpeg(&self, quality: u8) -> Vec<u8> {
        let straight = self.to_straight();
        let rgb: Vec<u8> = straight.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
        let Some(img) = image::RgbImage::from_raw(self.width, self.height, rgb) else { return Vec::new() };
        let mut buf = Vec::new();
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.clamp(1, 100));
        if let Err(e) = img.write_with_encoder(enc) {
            log::error!("JPEG encode: {e}");
            return Vec::new();
        }
        buf
    }
    /// Pixel at (x, y) as straight RGBA.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let p = self.pixels.get(i..i + 4).unwrap_or(&[0, 0, 0, 0]);
        let a = p.get(3).copied().unwrap_or(0) as u32;
        let un = |c: u8| (c as u32 * 255 + a / 2).checked_div(a).map(|v| v.min(255) as u8).unwrap_or(0);
        [un(p[0]), un(p[1]), un(p[2]), a as u8]
    }
}
