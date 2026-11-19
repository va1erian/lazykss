//! Object (enemy, item) sprites, drawn from the ROM.
//!
//! Object graphics are not in the resource table the tilesets use. Each
//! enemy family has a compressed *package*:
//!
//! ```text
//! +0    1 byte    slot class bits (which OBJ palette row / VRAM area it uses)
//! +1    5 x 32    palettes: 0F, CGRAM index, 15 BGR555 words
//!                 (0 normal, 1-3 alternates, 4 the hit flash)
//! +161  n x 32    4bpp characters
//! ```
//!
//! Frames are lists of 5-byte records in the ROM (`flags, dx, dy, character,
//! attributes`; flag bit 0 = 16x16, bit 1 = last piece), reached through
//! object code rather than a table, so which package and frame an object id
//! uses is recorded in the generated [`sprite_table`](super::sprite_table).

use crate::compression;
use crate::gfx::{Image, bgr555};
use crate::rom::Rom;
use crate::sprite_table::{Colors, OBJECTS, PALETTES, Piece, SOURCES};

/// An object's picture and where its position falls inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite {
    pub image: Image,
    /// Pixel of `image` that sits on the object's `(x, y)`.
    pub origin_x: i32,
    pub origin_y: i32,
}

/// The sprites of every object the table knows, rendered once.
#[derive(Debug, Clone, Default)]
pub struct ObjectSprites {
    /// Parallel to `OBJECTS`.
    sprites: Vec<Option<Sprite>>,
}

impl ObjectSprites {
    /// Decodes every known object from `rom`. An object whose data does not
    /// decode (a different ROM revision, a hack that moved it) has no sprite.
    pub fn load(rom: &Rom) -> Self {
        let sources: Vec<Vec<u8>> = SOURCES
            .iter()
            .map(|&(offset, _)| {
                compression::decompress(&rom.data()[offset as usize..])
                    .map(|(data, _)| data)
                    .unwrap_or_default()
            })
            .collect();
        let palettes: Vec<Option<[[u8; 4]; 16]>> =
            PALETTES.iter().map(|&c| colors(rom, &sources, c)).collect();
        let sprites = OBJECTS
            .iter()
            .map(|&(_, _, pieces)| render(&sources, &palettes, pieces))
            .collect();
        Self { sprites }
    }

    /// The sprite of an object: the exact variant when it is known, else any
    /// variant of the same id.
    pub fn get(&self, id: u8, variant: u8) -> Option<&Sprite> {
        let exact = OBJECTS
            .iter()
            .position(|&(i, v, _)| i == id && v == variant);
        let any = || OBJECTS.iter().position(|&(i, _, _)| i == id);
        self.sprites.get(exact.or_else(any)?)?.as_ref()
    }

    /// Ids and variants that have a sprite.
    pub fn known(&self) -> impl Iterator<Item = (u8, u8)> + '_ {
        OBJECTS
            .iter()
            .zip(&self.sprites)
            .filter(|(_, s)| s.is_some())
            .map(|(&(id, variant, _), _)| (id, variant))
    }
}

fn colors(rom: &Rom, sources: &[Vec<u8>], colors: Colors) -> Option<[[u8; 4]; 16]> {
    let words: &[u8] = match colors {
        Colors::Package { source, index } => {
            let at = 3 + 32 * index as usize;
            sources[source as usize].get(at..at + 30)?
        }
        Colors::Raw(offset) => rom.data().get(offset as usize..offset as usize + 30)?,
    };
    let mut palette = [[0u8; 4]; 16];
    for (color, word) in palette[1..].iter_mut().zip(words.chunks_exact(2)) {
        *color = bgr555(u16::from_le_bytes([word[0], word[1]]));
    }
    Some(palette)
}

fn render(
    sources: &[Vec<u8>],
    palettes: &[Option<[[u8; 4]; 16]>],
    pieces: &[Piece],
) -> Option<Sprite> {
    let left = pieces.iter().map(|p| p.0).min()? as i32;
    let top = pieces.iter().map(|p| p.1).min()? as i32;
    let right = pieces.iter().map(|p| p.0).max()? as i32 + 8;
    let bottom = pieces.iter().map(|p| p.1).max()? as i32 + 8;
    let mut image = Image::new((right - left) as usize, (bottom - top) as usize);
    // The first piece is in front: draw back to front.
    for &(dx, dy, flip, source, character, palette) in pieces.iter().rev() {
        let palette = palettes[palette as usize].as_ref()?;
        let start = SOURCES[source as usize].1 as usize + character as usize * 32;
        let tile = sources[source as usize].get(start..start + 32)?;
        for y in 0..8usize {
            let row = if flip & 2 != 0 { 7 - y } else { y };
            let planes = [
                tile[row * 2],
                tile[row * 2 + 1],
                tile[16 + row * 2],
                tile[17 + row * 2],
            ];
            for x in 0..8usize {
                let shift = if flip & 1 != 0 { x } else { 7 - x };
                let value = planes
                    .iter()
                    .enumerate()
                    .fold(0, |acc, (bit, p)| acc | ((p >> shift) & 1) << bit);
                if value != 0 {
                    let px = (dx as i32 - left) as usize + x;
                    let py = (dy as i32 - top) as usize + y;
                    let at = (py * image.width + px) * 4;
                    image.rgba[at..at + 4].copy_from_slice(&palette[value as usize]);
                }
            }
        }
    }
    Some(Sprite {
        image,
        origin_x: -left,
        origin_y: -top,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_in_range() {
        assert!(
            OBJECTS
                .windows(2)
                .all(|w| (w[0].0, w[0].1) < (w[1].0, w[1].1))
        );
        for &(_, _, pieces) in OBJECTS {
            assert!(!pieces.is_empty());
            for &(_, _, flip, source, _, palette) in pieces {
                assert!(flip < 4);
                assert!((source as usize) < SOURCES.len());
                assert!((palette as usize) < PALETTES.len());
            }
        }
    }

    #[test]
    fn every_listed_object_decodes() {
        let Some(path) = Rom::locate() else { return };
        let rom = Rom::load(&path).unwrap();
        let sprites = ObjectSprites::load(&rom);
        assert_eq!(sprites.known().count(), OBJECTS.len());
        // Waddle Dee: a 24x24 sprite standing on its position, orange body.
        let dee = sprites.get(0x01, 0x00).unwrap();
        assert_eq!((dee.image.width, dee.image.height), (24, 24));
        let opaque = dee
            .image
            .rgba
            .chunks_exact(4)
            .filter(|p| p[3] == 255)
            .count();
        assert!(opaque > 250, "{opaque} opaque pixels");
        // An unknown variant falls back to a known one; an unknown id has none.
        assert!(sprites.get(0x01, 0x77).is_some());
        assert!(sprites.get(0xEE, 0).is_none());
    }
}
