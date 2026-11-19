//! Turning `kss-core` images into xui images, with nearest-neighbour zoom.
//!
//! xui's `Canvas::draw_image` resamples bilinearly when it scales, which
//! blurs pixel art. Zoomed views therefore cut the room image into chunks,
//! scale each by an integer factor here and draw the result 1:1 (which the
//! canvas blits as a plain row copy). Chunks are built when first visible and
//! kept until the image or the zoom changes.

use std::collections::HashMap;

use kss_core::{Image as Raw, ObjectSprites};
use xui::image::Image;

/// Edge of a chunk, in source pixels.
pub const CHUNK: usize = 128;

/// A whole `kss-core` image as an xui image (no scaling).
pub fn to_xui(raw: &Raw) -> Option<Image> {
    Image::from_rgba(raw.width as u32, raw.height as u32, raw.rgba.clone()).ok()
}

/// The source rectangle `(x, y, w, h)` scaled by `zoom` with nearest-neighbour
/// sampling, clipped to the image.
pub fn zoomed(raw: &Raw, x: usize, y: usize, w: usize, h: usize, zoom: usize) -> Option<Image> {
    let w = w.min(raw.width.saturating_sub(x));
    let h = h.min(raw.height.saturating_sub(y));
    if w == 0 || h == 0 || zoom == 0 {
        return None;
    }
    let (ow, oh) = (w * zoom, h * zoom);
    let mut out = vec![0u8; ow * oh * 4];
    for sy in 0..h {
        let src_row = ((y + sy) * raw.width + x) * 4;
        let first = sy * zoom * ow * 4;
        for sx in 0..w {
            let px = &raw.rgba[src_row + sx * 4..src_row + sx * 4 + 4];
            for zx in 0..zoom {
                let at = first + (sx * zoom + zx) * 4;
                out[at..at + 4].copy_from_slice(px);
            }
        }
        // Repeat the first output row for the remaining `zoom - 1` rows.
        for zy in 1..zoom {
            out.copy_within(first..first + ow * 4, first + zy * ow * 4);
        }
    }
    Image::from_rgba(ow as u32, oh as u32, out).ok()
}

/// A source image plus its zoomed chunks.
pub struct ChunkCache {
    source: Option<Raw>,
    /// Identifies what `source` was rendered from.
    pub key: u64,
    zoom: usize,
    chunks: HashMap<(usize, usize), Image>,
    /// Object sprites scaled to a zoom, keyed by `(id, variant, zoom)`.
    sprites: HashMap<(u8, u8, usize), Option<Image>>,
}

impl ChunkCache {
    pub fn new() -> Self {
        Self {
            source: None,
            key: u64::MAX,
            zoom: 0,
            chunks: HashMap::new(),
            sprites: HashMap::new(),
        }
    }

    /// Replaces the source image; chunks are dropped.
    pub fn set_source(&mut self, key: u64, raw: Raw) {
        self.source = Some(raw);
        self.key = key;
        self.chunks.clear();
    }

    /// The object's sprite scaled by `zoom`, built on first use.
    pub fn sprite(
        &mut self,
        sprites: &ObjectSprites,
        id: u8,
        variant: u8,
        zoom: usize,
    ) -> Option<&Image> {
        self.sprites
            .entry((id, variant, zoom))
            .or_insert_with(|| {
                let raw = &sprites.get(id, variant)?.image;
                zoomed(raw, 0, 0, raw.width, raw.height, zoom)
            })
            .as_ref()
    }

    /// The chunk at chunk coordinates `(cx, cy)` at `zoom`, built on demand.
    pub fn chunk(&mut self, cx: usize, cy: usize, zoom: usize) -> Option<&Image> {
        if self.zoom != zoom {
            self.zoom = zoom;
            self.chunks.clear();
        }
        if !self.chunks.contains_key(&(cx, cy)) {
            let raw = self.source.as_ref()?;
            let image = zoomed(raw, cx * CHUNK, cy * CHUNK, CHUNK, CHUNK, zoom)?;
            self.chunks.insert((cx, cy), image);
        }
        self.chunks.get(&(cx, cy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_repeats_pixels_without_blending() {
        let mut raw = Raw::filled(3, 2, [0, 0, 0, 255]);
        raw.rgba[0..4].copy_from_slice(&[255, 0, 0, 255]);
        raw.rgba[(3 + 2) * 4..(3 + 2) * 4 + 4].copy_from_slice(&[0, 0, 255, 255]);
        let z = zoomed(&raw, 0, 0, 3, 2, 3).unwrap();
        assert_eq!(z.size(), (9, 6));
        for (x, y) in [(0, 0), (2, 2), (1, 1)] {
            assert_eq!(z.pixel(x, y), Some([255, 0, 0, 255]));
        }
        assert_eq!(z.pixel(3, 0), Some([0, 0, 0, 255]));
        for (x, y) in [(6, 3), (8, 5)] {
            assert_eq!(z.pixel(x, y), Some([0, 0, 255, 255]));
        }
        assert_eq!(z.pixel(5, 5), Some([0, 0, 0, 255]));
    }

    #[test]
    fn chunks_clip_to_the_image_edge() {
        let mut cache = ChunkCache::new();
        cache.set_source(1, Raw::filled(130, 10, [1, 2, 3, 255]));
        assert_eq!(cache.chunk(0, 0, 2).unwrap().size(), (256, 20));
        assert_eq!(cache.chunk(1, 0, 2).unwrap().size(), (4, 20));
        assert!(cache.chunk(0, 1, 2).is_none());
    }
}
