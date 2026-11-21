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
use crate::gfx::{CHAR_BYTES, Image, bgr555, char_pixels, decode_char, le_words};
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
                usize::try_from(offset)
                    .ok()
                    .and_then(|at| rom.data().get(at..))
                    .and_then(|data| compression::decompress(data).ok())
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

/// `len` bytes at `at` of `data`, when they are all there.
fn span(data: &[u8], at: usize, len: usize) -> Option<&[u8]> {
    data.get(at..)?.get(..len)
}

/// Bytes of the 15 colours of a palette.
const COLOR_BYTES: usize = 30;
/// Bytes of one package palette: `0F`, the CGRAM index, then the colours.
const PACKAGE_PALETTE: usize = 32;
/// Where the colours of package palette 0 start: after the slot class byte
/// and the two leading bytes of the palette.
const PACKAGE_COLORS: usize = 3;

fn colors(rom: &Rom, sources: &[Vec<u8>], colors: Colors) -> Option<[[u8; 4]; 16]> {
    let words: &[u8] = match colors {
        Colors::Package { source, index } => {
            let at = usize::from(index)
                .checked_mul(PACKAGE_PALETTE)?
                .checked_add(PACKAGE_COLORS)?;
            span(sources.get(usize::from(source))?, at, COLOR_BYTES)?
        }
        Colors::Raw(offset) => span(rom.data(), usize::try_from(offset).ok()?, COLOR_BYTES)?,
    };
    // Entry 0 is transparent: the stored colours are entries 1 to 15.
    let mut palette = [[0u8; 4]; 16];
    for (color, word) in palette.iter_mut().skip(1).zip(le_words(words)) {
        *color = bgr555(word);
    }
    Some(palette)
}

/// The box the pieces cover: left, top, right and bottom edges, in pixels
/// from the object's position.
fn bounds(pieces: &[Piece]) -> Option<(i32, i32, i32, i32)> {
    let left = i32::from(pieces.iter().map(|p| p.0).min()?);
    let top = i32::from(pieces.iter().map(|p| p.1).min()?);
    // Each piece is one 8x8 character; i16 offsets plus 8 always fit i32.
    let right = i32::from(pieces.iter().map(|p| p.0).max()?).saturating_add(8);
    let bottom = i32::from(pieces.iter().map(|p| p.1).max()?).saturating_add(8);
    Some((left, top, right, bottom))
}

/// Distance from `origin` to `at`, when `at` is not before it.
fn distance(at: i16, origin: i32) -> Option<usize> {
    usize::try_from(i32::from(at).saturating_sub(origin)).ok()
}

fn render(
    sources: &[Vec<u8>],
    palettes: &[Option<[[u8; 4]; 16]>],
    pieces: &[Piece],
) -> Option<Sprite> {
    let (left, top, right, bottom) = bounds(pieces)?;
    let width = usize::try_from(right.saturating_sub(left)).ok()?;
    let height = usize::try_from(bottom.saturating_sub(top)).ok()?;
    let mut image = Image::new(width, height);
    // The first piece is in front: draw back to front.
    for &(dx, dy, flip, source, character, palette) in pieces.iter().rev() {
        let palette = palettes.get(usize::from(palette))?.as_ref()?;
        let source = usize::from(source);
        let start = usize::from(character)
            .checked_mul(CHAR_BYTES)?
            .checked_add(SOURCES.get(source)?.1.into())?;
        let char = decode_char(sources.get(source)?.get(start..)?)?;
        let (x0, y0) = (distance(dx, left)?, distance(dy, top)?);
        for (x, y, value) in char_pixels(&char, flip & 1 != 0, flip & 2 != 0) {
            if let Some(&color) = palette.get(usize::from(value)).filter(|_| value != 0) {
                image.put(x0.saturating_add(x), y0.saturating_add(y), color);
            }
        }
    }
    Some(Sprite {
        image,
        origin_x: left.saturating_neg(),
        origin_y: top.saturating_neg(),
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
