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

use crate::compression;
use crate::room::{CELL, Room, Tables};
use crate::rom::Rom;

const CHARS: usize = 0x800;
const BG_CHAR_BASE: usize = 0x280;
const PLANES: usize = 9;

/// An RGBA8 image, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, rgba: vec![0; width * height * 4] }
    }

    pub fn filled(width: usize, height: usize, color: [u8; 4]) -> Self {
        Self { width, height, rgba: color.repeat(width * height) }
    }

    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let at = (y * self.width + x) * 4;
        self.rgba[at..at + 4].try_into().expect("4 bytes")
    }

    fn put(&mut self, x: usize, y: usize, color: [u8; 4]) {
        if x < self.width && y < self.height {
            let at = (y * self.width + x) * 4;
            self.rgba[at..at + 4].copy_from_slice(&color);
        }
    }
}

/// Which parts of a room [`Tileset::render_room`] draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layers {
    pub background: bool,
    pub foreground: bool,
}

impl Default for Layers {
    fn default() -> Self {
        Self { background: true, foreground: true }
    }
}

/// Converts a BGR555 colour word to opaque RGBA.
pub fn bgr555(word: u16) -> [u8; 4] {
    let channel = |c: u16| ((c & 31) << 3 | (c & 31) >> 2) as u8;
    [channel(word), channel(word >> 5), channel(word >> 10), 255]
}

/// Everything needed to draw a room's two tile layers.
#[derive(Debug, Clone)]
pub struct Tileset {
    /// Palette index (0-15) of every pixel of every character, 64 per
    /// character.
    pixels: Vec<u8>,
    palette: [[u8; 4]; 256],
    /// `[position][metatile]` tilemap words.
    planes: Vec<Vec<u16>>,
    attributes: Vec<u8>,
    bg_width: usize,
    bg_height: usize,
    bg_map: Vec<u16>,
}

impl Tileset {
    /// Loads the resources a graphics header names. A missing or malformed
    /// resource leaves its part empty: the rest still draws.
    pub fn load(rom: &Rom, tables: &Tables, header: &[u8; 24]) -> Self {
        let resource = |index: usize| tables.resource(rom, index);
        let unpack = |index: usize| {
            resource(index)
                .and_then(|at| compression::decompress(&rom.data()[at..]).ok())
                .map(|(data, _)| data)
                .unwrap_or_default()
        };

        let mut palette = [[0, 0, 0, 255]; 256];
        for index in [header[19], header[3], header[8]] {
            let Some(at) = resource(index as usize) else { continue };
            let (count, first) = (rom.u8(at) as usize, rom.u8(at + 1) as usize);
            for i in 0..count {
                if first + i < 256 && at + 4 + i * 2 <= rom.data().len() {
                    palette[first + i] = bgr555(rom.u16(at + 2 + i * 2));
                }
            }
        }

        let mut chars = vec![0u8; CHARS * 32];
        let mut place = |data: &[u8], first: usize| {
            let room = chars.len() - first * 32;
            let len = data.len().min(room);
            chars[first * 32..first * 32 + len].copy_from_slice(&data[..len]);
        };
        // The foreground set is loaded last: it overlaps the start of the
        // background set.
        place(&unpack(header[7] as usize), BG_CHAR_BASE);
        place(&unpack(header[2] as usize), 0);
        let mut pixels = vec![0u8; CHARS * 64];
        for (tile, out) in chars.chunks_exact(32).zip(pixels.chunks_exact_mut(64)) {
            for y in 0..8 {
                let rows = [tile[y * 2], tile[y * 2 + 1], tile[16 + y * 2], tile[17 + y * 2]];
                for x in 0..8 {
                    let shift = 7 - x;
                    out[y * 8 + x] =
                        rows.iter().enumerate().fold(0, |acc, (bit, row)| acc | ((row >> shift) & 1) << bit);
                }
            }
        }

        let table = unpack(header[5] as usize | (header[6] as usize) << 8);
        let count = table.len() / (PLANES * 2 + 1);
        let planes = (0..PLANES)
            .map(|p| {
                table[p * count * 2..(p + 1) * count * 2]
                    .chunks_exact(2)
                    .map(|w| u16::from_le_bytes([w[0], w[1]]))
                    .collect()
            })
            .collect();
        let attributes = table[PLANES * count * 2..PLANES * count * 2 + count].to_vec();

        let (mut bg_width, mut bg_height, mut bg_map) = (0, 0, Vec::new());
        if let Some(at) = resource(header[10] as usize) {
            let (w, h) = (rom.u16(at) as usize, rom.u16(at + 2) as usize);
            if let Ok((data, _)) = compression::decompress(&rom.data()[at + 4..]) {
                if w > 0 && h > 0 && data.len() >= w * h * 2 {
                    bg_map = data.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                    (bg_width, bg_height) = (w, h);
                }
            }
        }

        Self { pixels, palette, planes, attributes, bg_width, bg_height, bg_map }
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
        self.attributes.get(metatile as usize).copied().unwrap_or(0)
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

    /// True when every character of the metatile is fully transparent.
    pub fn is_blank(&self, metatile: u16) -> bool {
        let Some(words) = self.words(metatile) else { return true };
        words.iter().all(|w| {
            let c = (w & 0x3FF) as usize;
            self.pixels[c * 64..c * 64 + 64].iter().all(|&p| p == 0)
        })
    }

    fn words(&self, metatile: u16) -> Option<[u16; PLANES]> {
        let m = metatile as usize;
        (m < self.metatile_count()).then(|| std::array::from_fn(|p| self.planes[p][m]))
    }

    /// Draws one 8x8 character from a tilemap word. Colour 0 is skipped
    /// unless `opaque`.
    fn draw_char(&self, image: &mut Image, x0: isize, y0: isize, word: u16, opaque: bool) {
        let c = (word & 0x3FF) as usize;
        let row = ((word >> 10) & 7) as usize * 16;
        let (hflip, vflip) = (word & 0x4000 != 0, word & 0x8000 != 0);
        let pixels = &self.pixels[c * 64..c * 64 + 64];
        for y in 0..8 {
            let py = y0 + y as isize;
            if py < 0 {
                continue;
            }
            let sy = if vflip { 7 - y } else { y };
            for x in 0..8 {
                let px = x0 + x as isize;
                let sx = if hflip { 7 - x } else { x };
                let value = pixels[sy * 8 + sx];
                if px >= 0 && (value != 0 || opaque) {
                    image.put(px as usize, py as usize, self.palette[row + value as usize]);
                }
            }
        }
    }

    /// Draws a metatile with its top-left corner at `(x, y)`.
    pub fn draw_metatile(&self, image: &mut Image, x: isize, y: isize, metatile: u16) {
        let Some(words) = self.words(metatile) else { return };
        for (i, word) in words.into_iter().enumerate() {
            self.draw_char(image, x + (i % 3 * 8) as isize, y + (i / 3 * 8) as isize, word, false);
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
        let mut image = Image::filled(columns * CELL, rows * CELL, fill);
        for id in 0..self.metatile_count() {
            let (x, y) = ((id % columns * CELL) as isize, (id / columns * CELL) as isize);
            self.draw_metatile(&mut image, x, y, id as u16);
        }
        image
    }

    /// The background layer tiled over a `width` x `height` pixel area. The
    /// game scrolls it at its own rate; here it is pinned to the origin.
    pub fn draw_background(&self, image: &mut Image) {
        if self.bg_map.is_empty() {
            return;
        }
        for ty in 0..image.height.div_ceil(8) {
            for tx in 0..image.width.div_ceil(8) {
                let word = self.bg_map[ty % self.bg_height * self.bg_width + tx % self.bg_width];
                self.draw_char(image, (tx * 8) as isize, (ty * 8) as isize, word, false);
            }
        }
    }

    /// Renders a whole room at one pixel per SNES pixel.
    pub fn render_room(&self, room: &Room, layers: Layers) -> Image {
        let mut image = Image::filled(room.width * CELL, room.height * CELL, self.backdrop());
        if layers.background {
            self.draw_background(&mut image);
        }
        if layers.foreground {
            for y in 0..room.height {
                for x in 0..room.width {
                    self.draw_metatile(&mut image, (x * CELL) as isize, (y * CELL) as isize, room.tile(x, y));
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
            [0x1000, 0x1000, 0x1000, 0x1000, 0x1000, 0x1000, 0x103A, 0x103A, 0x103A]
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
