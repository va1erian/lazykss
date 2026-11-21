//! Room tables, room parsing and the save pipeline.
//!
//! A room block, at the address its entry in the room pointer table gives:
//!
//! ```text
//! +0   17 bytes  misc header (music at +2, scroll rate at +6, ...)
//! +17   5 bytes  FF padding
//! +22  24 bytes  graphics header (see the `gfx` module)
//! +46   1 byte   fifth object graphics slot
//! +47   7 bytes  99 01 01 01 01 01 99
//! +54   4 bytes  two words, unknown (historically called "entrance")
//! +58            six sections, each starting with a 16-bit pointer relative
//!                to itself that leads to the next one. Section 0 holds the
//!                door list: 10-byte records up to an FFFF terminator; a
//!                door's destination counts rooms from the start of the
//!                level the room belongs to.
//!                Section 3 lists the breakable cells (`Block`).
//! then  2 bytes  3DEF in every room
//!       2 bytes  width in metatiles
//!       2 bytes  height in metatiles
//!       ...      HAL-compressed blob:
//!                  width*height tile-id low bytes, row-major
//!                  width*height tile-id high bytes
//!                  6-byte object records, FFFF terminator
//! ```
//!
//! Metatiles are 24x24 pixels (3x3 SNES characters); object and door
//! coordinates are pixels from the room's top-left corner.
//!
//! Every offset and length here comes from the ROM, which is untrusted: a
//! value that leads outside the data is an error (or an empty result), never
//! a panic.

mod blocks;
mod bytes;
mod doors;
mod error;
mod load;
mod objects;
mod save;
mod subgames;
mod tables;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_edit;
#[cfg(test)]
mod tests_subgames;

pub use blocks::Block;
pub use doors::{Destination, Door};
pub use error::RoomError;
pub use objects::Object;
pub use save::{SaveReport, free_space};
pub use subgames::{FIRST_UNPLAYED_LEVEL, SubGame};
pub use tables::{GREAT_CAVE_OFFENSIVE, Level, Tables};

/// Pixel size of one map cell.
pub const CELL: usize = 24;
/// Most doors a room can list: a warp star names its door with one byte.
pub const MAX_DOORS: usize = 255;
/// Largest map [`Room::encode`] accepts: both tile planes and a full object
/// list must fit the 64 KiB a compressed stream can hold.
pub const MAX_CELLS: usize = 30_000;
/// Offset in the room block of the first section.
const SECTIONS_OFFSET: usize = 58;
const SECTION_COUNT: usize = 6;
const MISC_MUSIC: usize = 2;
/// Where the graphics header sits in the room block.
const GFX_HEADER: std::ops::Range<usize> = 22..46;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub index: usize,
    /// File offset of the room block.
    pub offset: usize,
    /// Size in the ROM of the block as loaded, compressed blob included.
    pub stored_len: usize,
    /// Everything before the compressed blob, verbatim. The typed accessors
    /// read and write through it, so unknown fields round-trip untouched.
    head: Vec<u8>,
    /// Offset in `head` of the width word.
    size_at: usize,
    /// Offset in `head` just past the last door slot.
    doors_end: usize,
    pub width: usize,
    pub height: usize,
    /// `width * height` metatile ids, row-major.
    pub tiles: Vec<u16>,
    pub objects: Vec<Object>,
    /// Bytes after the object terminator, kept verbatim (normally empty).
    tail: Vec<u8>,
}

impl Room {
    /// The raw bytes before the compressed blob.
    pub fn head(&self) -> &[u8] {
        &self.head
    }

    /// Overwrites header bytes. `offset` is relative to the room block; the
    /// write is clipped to the fixed part before the door list.
    pub fn patch_head(&mut self, offset: usize, bytes: &[u8]) {
        let fixed = self.head.iter_mut().take(SECTIONS_OFFSET).skip(offset);
        for (dst, &src) in fixed.zip(bytes) {
            *dst = src;
        }
    }

    pub fn music(&self) -> u8 {
        // A loaded head always holds the fixed part, so the fallback is
        // only there to keep this total.
        self.head.get(MISC_MUSIC).copied().unwrap_or_default()
    }

    pub fn set_music(&mut self, song: u8) {
        if let Some(music) = self.head.get_mut(MISC_MUSIC) {
            *music = song;
        }
    }

    /// The 24-byte graphics header.
    pub fn gfx_header(&self) -> [u8; 24] {
        self.head
            .get(GFX_HEADER)
            .and_then(|bytes| bytes.try_into().ok())
            .unwrap_or([0; 24])
    }

    pub fn set_gfx_header(&mut self, header: [u8; 24]) {
        if let Some(bytes) = self.head.get_mut(GFX_HEADER) {
            bytes.copy_from_slice(&header);
        }
    }

    /// Index in `tiles` of the cell at `(x, y)`, before any bounds check.
    fn cell_index(&self, x: usize, y: usize) -> Option<usize> {
        y.checked_mul(self.width)?.checked_add(x)
    }

    /// The metatile at `(x, y)`; the empty tile outside the map.
    pub fn tile(&self, x: usize, y: usize) -> u16 {
        self.cell_index(x, y)
            .and_then(|i| self.tiles.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// Sets the metatile at `(x, y)`; a cell outside the map is ignored.
    pub fn set_tile(&mut self, x: usize, y: usize, id: u16) {
        if let Some(tile) = self.cell_index(x, y).and_then(|i| self.tiles.get_mut(i)) {
            *tile = id;
        }
    }

    /// Resizes the map, keeping the top-left cells and filling new ones with
    /// the empty tile. A size whose cell count overflows is ignored.
    pub fn resize(&mut self, width: usize, height: usize) {
        if width.checked_mul(height).is_none() {
            return;
        }
        // Block records address cells by index, which depends on the width.
        let blocks = self.blocks();
        let tiles = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .map(|(x, y)| {
                if x < self.width && y < self.height {
                    self.tile(x, y)
                } else {
                    0
                }
            })
            .collect();
        self.tiles = tiles;
        self.width = width;
        self.height = height;
        // A head whose section words do not add up keeps its old records,
        // as it would keep any other part of it.
        if !blocks.is_empty() {
            self.set_blocks(&blocks);
        }
    }
}
