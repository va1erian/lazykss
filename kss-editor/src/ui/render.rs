//! Turning `kss-core` images into xui images.
//!
//! Pixel art is drawn through `Canvas::draw_image_sampled` with nearest
//! neighbour sampling, so the images are kept at one pixel per SNES pixel
//! and scaled by the canvas.

use std::collections::HashMap;

use kss_core::{Image as Raw, ObjectSprites, Tileset};
use xui::image::Image;

/// A whole `kss-core` image as an xui image.
pub fn to_xui(raw: &Raw) -> Option<Image> {
    Image::from_rgba(raw.width as u32, raw.height as u32, raw.rgba.clone()).ok()
}

/// The rendered room and the object sprites, as xui images.
pub struct RoomImages {
    room: Option<Image>,
    /// Identifies what `room` was rendered from.
    pub key: u64,
    /// The opened ROM the sprites below were decoded from.
    session: u64,
    /// Object sprites by `(id, variant)`, converted on first use.
    sprites: HashMap<(u8, u8), Option<Image>>,
    /// The session and room the metatiles below were drawn from.
    tiles_key: (u64, usize),
    /// Single metatiles, for the buffer and the stamp, converted on first use.
    tiles: HashMap<u16, Option<Image>>,
}

impl RoomImages {
    pub fn new() -> Self {
        Self {
            room: None,
            key: u64::MAX,
            session: u64::MAX,
            sprites: HashMap::new(),
            tiles_key: (u64::MAX, usize::MAX),
            tiles: HashMap::new(),
        }
    }

    /// Another ROM was opened: its sprites replace the cached ones.
    pub fn set_session(&mut self, session: u64) {
        if self.session != session {
            self.session = session;
            self.sprites.clear();
        }
    }

    /// One metatile of the room `key` (session, room index) on a
    /// transparent background.
    pub fn metatile(&mut self, key: (u64, usize), tileset: &Tileset, id: u16) -> Option<&Image> {
        if self.tiles_key != key {
            self.tiles_key = key;
            self.tiles.clear();
        }
        self.tiles
            .entry(id)
            .or_insert_with(|| to_xui(&tileset.metatile(id)))
            .as_ref()
    }

    /// Replaces the room image.
    pub fn set_room(&mut self, key: u64, raw: &Raw) {
        self.room = to_xui(raw);
        self.key = key;
    }

    pub fn room(&self) -> Option<&Image> {
        self.room.as_ref()
    }

    /// The object's sprite, when the ROM table has one.
    pub fn sprite(&mut self, sprites: &ObjectSprites, id: u8, variant: u8) -> Option<&Image> {
        self.sprites
            .entry((id, variant))
            .or_insert_with(|| to_xui(&sprites.get(id, variant)?.image))
            .as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_convert_pixel_for_pixel() {
        let mut raw = Raw::filled(3, 2, [0, 0, 0, 255]);
        raw.rgba[(3 + 2) * 4..(3 + 2) * 4 + 4].copy_from_slice(&[0, 0, 255, 255]);
        let image = to_xui(&raw).unwrap();
        assert_eq!(image.size(), (3, 2));
        assert_eq!(image.pixel(2, 1), Some([0, 0, 255, 255]));
        assert_eq!(image.pixel(0, 0), Some([0, 0, 0, 255]));
    }

    #[test]
    fn the_room_image_is_kept_until_replaced() {
        let mut images = RoomImages::new();
        assert!(images.room().is_none());
        images.set_room(1, &Raw::filled(4, 4, [1, 2, 3, 255]));
        assert_eq!((images.key, images.room().unwrap().size()), (1, (4, 4)));
    }
}
