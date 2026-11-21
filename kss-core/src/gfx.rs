//! Tileset loading and rendering.
//!
//! The 24-byte graphics header of a room names its resources as indices
//! into the resource pointer table (`Tables::resource`):
//!
//! | byte | resource | storage |
//! |---|---|---|
//! | 2 | foreground characters, loaded at character 0 | compressed 4bpp |
//! | 3 (= 4) | foreground palette | raw: count, first colour, BGR555 words |
//! | 5-6 | metatile table (16-bit index) | compressed, see below |
//! | 7 | background characters, loaded at character `0x280` | compressed 4bpp |
//! | 8 (= 9) | background palette | raw, as above |
//! | 10 | background tilemap | raw width and height words, then compressed |
//! | 19 | extra palette, loaded first | raw |
//! | 20-23 | object graphics slots (another id space) | |
//!
//! A metatile table holds `n` metatiles as ten planes: nine planes of `n`
//! tilemap words, one per position of the 3x3 block in row-major order, then
//! one plane of `n` attribute bytes. Words are SNES tilemap entries
//! (`vhopppcc cccccccc`).
//!
//! Both layers index one shared character space and one 256-colour palette,
//! as in VRAM/CGRAM while the room runs.

mod chars;
mod image;

pub(crate) use chars::{CHAR_BYTES, Char, char_pixels, decode_char, le_words};
pub use image::Image;

use crate::compression;
use crate::rom::Rom;
use crate::room::{CELL, Room, Tables};

const CHARS: usize = 0x800;
/// Byte offset of the background characters in the character space.
const BG_CHAR_OFFSET: usize = 0x280 * CHAR_BYTES;
const PLANES: usize = 9;
/// Top-left corner of each character of a metatile, in plane order.
const BLOCK_OFFSETS: [(isize, isize); PLANES] = [
    (0, 0),
    (8, 0),
    (16, 0),
    (0, 8),
    (8, 8),
    (16, 8),
    (0, 16),
    (8, 16),
    (16, 16),
];

/// Which parts of a room [`Tileset::render_room`] draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layers {
    pub background: bool,
    pub foreground: bool,
}

impl Default for Layers {
    fn default() -> Self {
        Self {
            background: true,
            foreground: true,
        }
    }
}

/// Converts a BGR555 colour word to opaque RGBA.
pub fn bgr555(word: u16) -> [u8; 4] {
    let channel = |c: u16| ((c & 31) << 3 | (c & 31) >> 2) as u8;
    [channel(word), channel(word >> 5), channel(word >> 10), 255]
}

/// `origin + delta` as an image coordinate; `None` left of or above the
/// image.
fn coordinate(origin: isize, delta: usize) -> Option<usize> {
    usize::try_from(origin.checked_add_unsigned(delta)?).ok()
}

/// Pixel position of tile `index` of `size` pixels.
fn tile_origin(index: usize, size: usize) -> isize {
    isize::try_from(index.saturating_mul(size)).unwrap_or(isize::MAX)
}

/// Everything needed to draw a room's two tile layers.
#[derive(Debug, Clone)]
pub struct Tileset {
    /// Every character of the shared character space.
    pixels: Vec<Char>,
    palette: [[u8; 4]; 256],
    /// `[position][metatile]` tilemap words.
    planes: Vec<Vec<u16>>,
    attributes: Vec<u8>,
    bg_width: usize,
    bg_height: usize,
    bg_map: Vec<u16>,
}

/// Applies a raw palette resource (count, first colour, BGR555 words).
/// Colours that would fall past entry 255 or past the ROM are skipped.
fn apply_palette(data: &[u8], palette: &mut [[u8; 4]; 256]) {
    let Some((&[count, first], words)) = data.split_first_chunk::<2>() else {
        return;
    };
    let colours = le_words(words).take(count.into()).map(bgr555);
    for (slot, colour) in palette.iter_mut().skip(first.into()).zip(colours) {
        *slot = colour;
    }
}

/// Splits a metatile table into its nine planes and its attribute bytes.
/// A short table gives as many whole metatiles as it holds.
fn split_metatiles(table: &[u8]) -> (Vec<Vec<u16>>, Vec<u8>) {
    let count = table.len() / (PLANES * 2 + 1);
    let plane_len = count.saturating_mul(2);
    let Some((words, rest)) = plane_len
        .checked_mul(PLANES)
        .and_then(|len| table.split_at_checked(len))
    else {
        return (vec![Vec::new(); PLANES], Vec::new());
    };
    let planes = if plane_len == 0 {
        vec![Vec::new(); PLANES]
    } else {
        words
            .chunks_exact(plane_len)
            .map(|plane| le_words(plane).collect())
            .collect()
    };
    let attributes = rest.get(..count).unwrap_or_default().to_vec();
    (planes, attributes)
}

/// Reads a background tilemap resource: width and height words, then a
/// compressed map of at least `width * height` words.
fn load_background(data: &[u8]) -> Option<(usize, usize, Vec<u16>)> {
    let (&[w0, w1, h0, h1], stream) = data.split_first_chunk::<4>()?;
    let width = usize::from(u16::from_le_bytes([w0, w1]));
    let height = usize::from(u16::from_le_bytes([h0, h1]));
    let (map, _) = compression::decompress(stream).ok()?;
    let needed = width.checked_mul(height)?.checked_mul(2)?;
    (width > 0 && height > 0 && map.len() >= needed)
        .then(|| (width, height, le_words(&map).collect()))
}

impl Tileset {
    /// Loads the resources a graphics header names. A missing or malformed
    /// resource leaves its part empty: the rest still draws.
    pub fn load(rom: &Rom, tables: &Tables, header: &[u8; 24]) -> Self {
        let resource = |index: usize| {
            tables
                .resource(rom, index)
                .and_then(|at| rom.data().get(at..))
        };
        let unpack = |index: usize| {
            resource(index)
                .and_then(|data| compression::decompress(data).ok())
                .map(|(data, _)| data)
                .unwrap_or_default()
        };

        let mut palette = [[0, 0, 0, 255]; 256];
        for index in [header[19], header[3], header[8]] {
            if let Some(data) = resource(index.into()) {
                apply_palette(data, &mut palette);
            }
        }

        let mut chars = vec![0u8; CHARS * CHAR_BYTES];
        let mut place = |data: &[u8], at: usize| {
            for (dst, src) in chars.iter_mut().skip(at).zip(data) {
                *dst = *src;
            }
        };
        // The foreground set is loaded last: it overlaps the start of the
        // background set.
        place(&unpack(header[7].into()), BG_CHAR_OFFSET);
        place(&unpack(header[2].into()), 0);
        let pixels = chars
            .chunks_exact(CHAR_BYTES)
            .filter_map(decode_char)
            .collect();

        let table = unpack(usize::from(header[5]) | usize::from(header[6]) << 8);
        let (planes, attributes) = split_metatiles(&table);

        let (bg_width, bg_height, bg_map) = resource(header[10].into())
            .and_then(load_background)
            .unwrap_or_default();

        Self {
            pixels,
            palette,
            planes,
            attributes,
            bg_width,
            bg_height,
            bg_map,
        }
    }

    /// Loads the tileset `room` uses.
    pub fn for_room(rom: &Rom, tables: &Tables, room: &Room) -> Self {
        Self::load(rom, tables, &room.gfx_header())
    }

    pub fn metatile_count(&self) -> usize {
        self.attributes.len()
    }

    /// The attribute (collision class) byte of a metatile.
    pub fn attribute(&self, metatile: u16) -> u8 {
        self.attributes
            .get(usize::from(metatile))
            .copied()
            .unwrap_or(0)
    }

    /// True for star, bomb and similar blocks (attribute `70`-`7F`). They
    /// are solid from the tile alone but only break when the room lists
    /// their cell (`Room::sync_blocks`).
    pub fn is_breakable(&self, metatile: u16) -> bool {
        (0x70..=0x7F).contains(&self.attribute(metatile))
    }

    pub fn palette(&self) -> &[[u8; 4]; 256] {
        &self.palette
    }

    /// The colour behind both layers.
    pub fn backdrop(&self) -> [u8; 4] {
        self.palette[0]
    }

    fn char(&self, word: u16) -> Option<&Char> {
        self.pixels.get(usize::from(word & 0x3FF))
    }

    /// True when every character of the metatile is fully transparent.
    pub fn is_blank(&self, metatile: u16) -> bool {
        let Some(words) = self.words(metatile) else {
            return true;
        };
        words
            .iter()
            .filter_map(|&w| self.char(w))
            .all(|c| c.iter().flatten().all(|&p| p == 0))
    }

    fn words(&self, metatile: u16) -> Option<[u16; PLANES]> {
        let m = usize::from(metatile);
        let mut words = [0; PLANES];
        for (word, plane) in words.iter_mut().zip(&self.planes) {
            *word = *plane.get(m)?;
        }
        Some(words)
    }

    /// Draws one 8x8 character from a tilemap word. Colour 0 is skipped
    /// unless `opaque`.
    fn draw_char(&self, image: &mut Image, x0: isize, y0: isize, word: u16, opaque: bool) {
        let Some(char) = self.char(word) else {
            return;
        };
        // Sixteen colours per palette row: the row number is the high nibble.
        let row = usize::from((word >> 10) & 7) << 4;
        let (hflip, vflip) = (word & 0x4000 != 0, word & 0x8000 != 0);
        for (x, y, value) in char_pixels(char, hflip, vflip) {
            if value == 0 && !opaque {
                continue;
            }
            let (Some(px), Some(py)) = (coordinate(x0, x), coordinate(y0, y)) else {
                continue;
            };
            if let Some(&colour) = self.palette.get(row | usize::from(value)) {
                image.put(px, py, colour);
            }
        }
    }

    /// Draws a metatile with its top-left corner at `(x, y)`.
    pub fn draw_metatile(&self, image: &mut Image, x: isize, y: isize, metatile: u16) {
        let Some(words) = self.words(metatile) else {
            return;
        };
        for (word, (dx, dy)) in words.into_iter().zip(BLOCK_OFFSETS) {
            let (cx, cy) = (x.saturating_add(dx), y.saturating_add(dy));
            self.draw_char(image, cx, cy, word, false);
        }
    }

    /// One metatile on a transparent 24x24 image.
    pub fn metatile(&self, metatile: u16) -> Image {
        let mut image = Image::new(CELL, CELL);
        self.draw_metatile(&mut image, 0, 0, metatile);
        image
    }

    /// Every metatile on one sheet, `columns` per row, over `fill`.
    /// Metatile `id` is at cell `(id % columns, id / columns)`.
    pub fn metatile_sheet(&self, columns: usize, fill: [u8; 4]) -> Image {
        let columns = columns.max(1);
        let rows = self.metatile_count().div_ceil(columns);
        let size = |cells: usize| cells.saturating_mul(CELL);
        let mut image = Image::filled(size(columns), size(rows), fill);
        let cells = (0..rows).flat_map(|row| (0..columns).map(move |column| (column, row)));
        let ids = (0..=u16::MAX).take(self.metatile_count());
        for (id, (column, row)) in ids.zip(cells) {
            let (x, y) = (tile_origin(column, CELL), tile_origin(row, CELL));
            self.draw_metatile(&mut image, x, y, id);
        }
        image
    }

    /// The background layer tiled over a `width` x `height` pixel area. The
    /// game scrolls it at its own rate; here it is pinned to the origin.
    pub fn draw_background(&self, image: &mut Image) {
        if self.bg_width == 0 {
            return;
        }
        let rows = self.bg_map.chunks_exact(self.bg_width).take(self.bg_height);
        let (columns, lines) = (image.width.div_ceil(8), image.height.div_ceil(8));
        for (ty, row) in (0..lines).zip(rows.cycle()) {
            for (tx, &word) in (0..columns).zip(row.iter().cycle()) {
                let (x, y) = (tile_origin(tx, 8), tile_origin(ty, 8));
                self.draw_char(image, x, y, word, false);
            }
        }
    }

    /// Renders a whole room at one pixel per SNES pixel.
    pub fn render_room(&self, room: &Room, layers: Layers) -> Image {
        let size = |cells: usize| cells.saturating_mul(CELL);
        let mut image = Image::filled(size(room.width), size(room.height), self.backdrop());
        if layers.background {
            self.draw_background(&mut image);
        }
        if layers.foreground {
            for y in 0..room.height {
                for x in 0..room.width {
                    let (px, py) = (tile_origin(x, CELL), tile_origin(y, CELL));
                    self.draw_metatile(&mut image, px, py, room.tile(x, y));
                }
            }
        }
        image
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_conversion() {
        assert_eq!(bgr555(0x001F), [255, 0, 0, 255]);
        assert_eq!(bgr555(0x03E0), [0, 255, 0, 255]);
        assert_eq!(bgr555(0x7C00), [0, 0, 255, 255]);
    }

    #[test]
    fn malformed_resources_do_not_panic() {
        assert_eq!(decode_char(&[0xFF; 31]), None);
        assert_eq!(decode_char(&[0xFF; 32]), Some([[15; 8]; 8]));
        let mut palette = [[0; 4]; 256];
        apply_palette(&[], &mut palette);
        apply_palette(&[0xFF, 0xF0, 0x1F, 0x00, 0x1F], &mut palette);
        assert_eq!(palette[0xF0], bgr555(0x1F));
        assert_eq!(palette[0xF1], [0; 4]);
        for len in [0, 1, 18, 19, 20, 57] {
            let (planes, attributes) = split_metatiles(&vec![1; len]);
            assert_eq!(planes.len(), PLANES);
            assert!(planes.iter().all(|p| p.len() == attributes.len()));
        }
        assert_eq!(load_background(&[1, 0, 1]), None);
        assert_eq!(load_background(&[0, 0, 1, 0, 0x00, 1, 0xFF]), None);
        assert_eq!(
            load_background(&[1, 0, 1, 0, 0x01, 1, 2, 0xFF]).unwrap().2,
            [0x201]
        );
        // The map promises more words than the stream holds.
        assert_eq!(load_background(&[2, 0, 1, 0, 0x01, 1, 2, 0xFF]), None);

        let mut image = Image::new(2, 2);
        image.put(2, 0, [9; 4]);
        image.put(usize::MAX, usize::MAX, [9; 4]);
        assert!(image.rgba.iter().all(|&b| b == 0));
        assert_eq!(image.pixel(5, 5), [0; 4]);
        let huge = Image::new(usize::MAX, 2);
        assert_eq!((huge.width, huge.height, huge.rgba.len()), (0, 0, 0));
    }

    #[test]
    fn green_greens_tileset() {
        let Some(path) = Rom::locate() else { return };
        let rom = Rom::load(&path).unwrap();
        let tables = Tables::load(&rom);
        let room = Room::load(&rom, &tables, 1).unwrap();
        let tiles = Tileset::for_room(&rom, &tables, &room);
        assert_eq!(tiles.metatile_count(), 720);
        // Ground top: two blank rows of characters, then the grass edge.
        assert_eq!(
            tiles.words(0x00A).unwrap(),
            [
                0x1000, 0x1000, 0x1000, 0x1000, 0x1000, 0x1000, 0x103A, 0x103A, 0x103A
            ]
        );
        assert_eq!(tiles.attribute(0x00C), 0x50);
        assert!(tiles.is_blank(0));
        assert!(!tiles.is_blank(0x00C));
        // Sky blue of the Green Greens background, as captured from CGRAM.
        assert_eq!(tiles.palette()[0x59], bgr555(0x7E87));
        let image = tiles.render_room(&room, Layers::default());
        assert_eq!((image.width, image.height), (2400, 192));
        // Top-left of the room is sky; the bottom row is ground.
        assert_eq!(image.pixel(100, 2), bgr555(0x7E87));
        assert_ne!(image.pixel(100, 190), bgr555(0x7E87));
    }
}
