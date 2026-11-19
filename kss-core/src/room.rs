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
//!                door list: 10-byte records up to an FFFF terminator.
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

use crate::compression::{self, DecompressError};
use crate::rom::Rom;

/// Pixel size of one map cell.
pub const CELL: usize = 24;
const MASTER_TABLE: usize = 0x3F_0000;
const SECTIONS_OFFSET: usize = 58;
const SECTION_COUNT: usize = 6;
const MISC_MUSIC: usize = 2;
const DOOR_SIZE: usize = 10;
/// The section that lists breakable cells.
const BLOCK_SECTION: usize = 3;
/// Free runs shorter than this are not worth tracking.
const MIN_FREE_RUN: usize = 0x200;
/// Bytes skipped at the start of a run of `FF`: the first ones can be the
/// terminators of the data that precedes it.
const FREE_GUARD: usize = 16;

#[derive(Debug)]
pub enum RoomError {
    BadIndex(usize),
    /// The room pointer or a section pointer leads outside the image.
    BadPointer { room: usize },
    Decompress { room: usize, source: DecompressError },
    /// The blob is shorter than two `width*height` planes.
    ShortMap { room: usize },
    /// No free block large enough to move the room to.
    NoFreeSpace { needed: usize },
    /// The map does not match `width*height`, or is too large to compress.
    BadDimensions,
    /// The compressor's output did not inflate back to the input.
    RoundTrip,
}

impl std::fmt::Display for RoomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadIndex(i) => write!(f, "room {i} does not exist"),
            Self::BadPointer { room } => write!(f, "room {room}: pointer outside the ROM"),
            Self::Decompress { room, source } => write!(f, "room {room}: {source}"),
            Self::ShortMap { room } => write!(f, "room {room}: map data is shorter than its size"),
            Self::NoFreeSpace { needed } => write!(f, "no free space for {needed} bytes"),
            Self::BadDimensions => write!(f, "tile map does not match the room size"),
            Self::RoundTrip => write!(f, "compression self-check failed"),
        }
    }
}

impl std::error::Error for RoomError {}

/// An enemy, item or mechanism placed in a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Object {
    /// Variant: an item's contents, an enemy's behaviour.
    pub variant: u8,
    pub id: u8,
    /// Pixels from the room's left edge.
    pub x: u16,
    /// Pixels from the room's top edge.
    pub y: u16,
}

/// A breakable cell: a record of section 3. A star, bomb or similar block
/// only reacts when its cell is listed here; without a record the metatile
/// is scenery Kirby walks through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub x: usize,
    pub y: usize,
    /// Metatile the cell turns into when the block breaks.
    pub after: u16,
    /// Third word of the record. In the original rooms it is the id of the
    /// cap metatile drawn in the cell above the block, and 0 for the cap's
    /// own record; what the game does with it is not established.
    pub extra: u16,
}

/// A door record of section 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Door {
    /// Index into the room pointer table.
    pub dest_room: u16,
    /// Trigger position in pixels. `FF00` on both axes marks an exit that is
    /// not entered by walking into it (a warp star, the end of a room).
    pub x: u16,
    pub y: u16,
    /// Where Kirby appears in the destination room.
    pub dest_x: u16,
    pub dest_y: u16,
}

impl Door {
    /// False for the exits that have no place in the room.
    pub fn has_trigger(&self) -> bool {
        self.x != 0xFF00 || self.y != 0xFF00
    }
}

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
    pub fn load(rom: &Rom, tables: &Tables, index: usize) -> Result<Self, RoomError> {
        let offset = *tables.room_offsets.get(index).ok_or(RoomError::BadIndex(index))?;
        let bad = RoomError::BadPointer { room: index };
        let len = rom.data().len();
        let mut section = offset + SECTIONS_OFFSET;
        for _ in 0..SECTION_COUNT {
            if section + 2 > len {
                return Err(bad);
            }
            let step = rom.u16(section) as usize;
            if step < 2 {
                return Err(bad);
            }
            section += step;
        }
        if section + 6 > len {
            return Err(bad);
        }
        let size_at = section + 2 - offset;
        let width = rom.u16(section + 2) as usize;
        let height = rom.u16(section + 4) as usize;
        let blob_at = section + 6;
        let (blob, packed_len) = compression::decompress(&rom.data()[blob_at..])
            .map_err(|source| RoomError::Decompress { room: index, source })?;
        let cells = width * height;
        if blob.len() < cells * 2 {
            return Err(RoomError::ShortMap { room: index });
        }
        let tiles = (0..cells).map(|i| blob[i] as u16 | (blob[cells + i] as u16) << 8).collect();
        let mut objects = Vec::new();
        let mut at = cells * 2;
        while at + 6 <= blob.len() && !(blob[at] == 0xFF && blob[at + 1] == 0xFF) {
            objects.push(Object {
                variant: blob[at],
                id: blob[at + 1],
                x: u16::from_le_bytes([blob[at + 2], blob[at + 3]]),
                y: u16::from_le_bytes([blob[at + 4], blob[at + 5]]),
            });
            at += 6;
        }
        let tail = blob.get(at + 2..).unwrap_or_default().to_vec();
        let head = rom.slice(offset, blob_at - offset).to_vec();
        let doors_end = SECTIONS_OFFSET + u16::from_le_bytes([head[58], head[59]]) as usize;
        Ok(Self {
            index,
            offset,
            stored_len: blob_at - offset + packed_len,
            head,
            size_at,
            doors_end: doors_end.min(size_at),
            width,
            height,
            tiles,
            objects,
            tail,
        })
    }

    /// The raw bytes before the compressed blob.
    pub fn head(&self) -> &[u8] {
        &self.head
    }

    /// Overwrites header bytes. `offset` is relative to the room block; the
    /// write is clipped to the fixed part before the door list.
    pub fn patch_head(&mut self, offset: usize, bytes: &[u8]) {
        let end = (offset + bytes.len()).min(SECTIONS_OFFSET);
        if offset < end {
            self.head[offset..end].copy_from_slice(&bytes[..end - offset]);
        }
    }

    pub fn music(&self) -> u8 {
        self.head[MISC_MUSIC]
    }

    pub fn set_music(&mut self, song: u8) {
        self.head[MISC_MUSIC] = song;
    }

    /// The 24-byte graphics header.
    pub fn gfx_header(&self) -> [u8; 24] {
        self.head[22..46].try_into().expect("24 bytes")
    }

    pub fn set_gfx_header(&mut self, header: [u8; 24]) {
        self.head[22..46].copy_from_slice(&header);
    }

    pub fn tile(&self, x: usize, y: usize) -> u16 {
        self.tiles[y * self.width + x]
    }

    pub fn set_tile(&mut self, x: usize, y: usize, id: u16) {
        self.tiles[y * self.width + x] = id;
    }

    fn door_slots(&self) -> impl Iterator<Item = usize> + '_ {
        (SECTIONS_OFFSET + 2..self.doors_end)
            .step_by(DOOR_SIZE)
            .take_while(|&at| at + DOOR_SIZE <= self.doors_end)
            .take_while(|&at| !(self.head[at] == 0xFF && self.head[at + 1] == 0xFF))
    }

    /// Start of section `index` in `head` and the offset just past it.
    fn section_range(&self, index: usize) -> (usize, usize) {
        let mut start = SECTIONS_OFFSET;
        for _ in 0..index {
            start += u16::from_le_bytes([self.head[start], self.head[start + 1]]) as usize;
        }
        (start, start + u16::from_le_bytes([self.head[start], self.head[start + 1]]) as usize)
    }

    /// The payload of section `index` (0-5), terminator included.
    pub fn section(&self, index: usize) -> &[u8] {
        let (start, end) = self.section_range(index.min(SECTION_COUNT - 1));
        &self.head[start + 2..end]
    }

    /// Replaces the payload of section `index`; the sections after it move.
    fn set_section(&mut self, index: usize, payload: &[u8]) {
        let (start, end) = self.section_range(index);
        let len = (payload.len() + 2) as u16;
        let mut section = len.to_le_bytes().to_vec();
        section.extend_from_slice(payload);
        let grown = section.len() as isize - (end - start) as isize;
        self.head.splice(start..end, section);
        self.size_at = (self.size_at as isize + grown) as usize;
        if index == 0 {
            self.doors_end = (self.doors_end as isize + grown) as usize;
        }
    }

    /// The breakable cells of the room.
    pub fn blocks(&self) -> Vec<Block> {
        self.section(BLOCK_SECTION)
            .chunks_exact(6)
            .take_while(|r| !(r[0] == 0xFF && r[1] == 0xFF))
            .map(|r| {
                let cell = u16::from_le_bytes([r[0], r[1]]) as usize / 2;
                Block {
                    x: cell % self.width.max(1),
                    y: cell / self.width.max(1),
                    after: u16::from_le_bytes([r[2], r[3]]),
                    extra: u16::from_le_bytes([r[4], r[5]]),
                }
            })
            .collect()
    }

    /// Replaces the list of breakable cells. Records are stored in map
    /// order, as the game's own rooms have them.
    pub fn set_blocks(&mut self, blocks: &[Block]) {
        let mut sorted: Vec<&Block> = blocks.iter().filter(|b| b.x < self.width && b.y < self.height).collect();
        sorted.sort_by_key(|b| (b.y, b.x));
        sorted.dedup_by_key(|b| (b.y, b.x));
        let mut payload = Vec::with_capacity(sorted.len() * 6 + 2);
        for block in sorted {
            payload.extend_from_slice(&(((block.y * self.width + block.x) * 2) as u16).to_le_bytes());
            payload.extend_from_slice(&block.after.to_le_bytes());
            payload.extend_from_slice(&block.extra.to_le_bytes());
        }
        payload.extend_from_slice(&[0xFF, 0xFF]);
        self.set_section(BLOCK_SECTION, &payload);
    }

    /// Brings the list of breakable cells in line with the map: a cell that
    /// now holds a `breakable` metatile and did not in `before` gets a record
    /// (breaking to the empty tile), and a record is dropped when its cell was a block in `before`
    /// (the tiles as last loaded or saved) and no longer is. Other records
    /// are left alone: the game's rooms also list decoration cells.
    ///
    /// Returns true when the list changed.
    pub fn sync_blocks(&mut self, breakable: impl Fn(u16) -> bool, before: &[u16]) -> bool {
        let old = self.blocks();
        let mut blocks: Vec<Block> = old
            .iter()
            .copied()
            .filter(|b| {
                let cell = b.y * self.width + b.x;
                let was_block = before.len() == self.tiles.len() && breakable(before[cell]);
                b.x < self.width && b.y < self.height && !(was_block && !breakable(self.tiles[cell]))
            })
            .collect();
        for y in 0..self.height {
            for x in 0..self.width {
                // Only blocks placed since `before`: a few original rooms
                // have unlisted blocks, and those stay as they are.
                let placed = before.get(y * self.width + x) != Some(&self.tile(x, y));
                if placed && breakable(self.tile(x, y)) && !blocks.iter().any(|b| (b.x, b.y) == (x, y)) {
                    blocks.push(Block { x, y, after: 0, extra: 0 });
                }
            }
        }
        blocks.sort_by_key(|b| (b.y, b.x));
        let changed = blocks != old;
        if changed {
            self.set_blocks(&blocks);
        }
        changed
    }

    pub fn doors(&self) -> Vec<Door> {
        let word = |at: usize| u16::from_le_bytes([self.head[at], self.head[at + 1]]);
        self.door_slots()
            .map(|at| Door {
                dest_room: word(at),
                x: word(at + 2),
                y: word(at + 4),
                dest_x: word(at + 6),
                dest_y: word(at + 8),
            })
            .collect()
    }

    /// Rewrites door `index` in place. The number of doors is fixed: the
    /// section that holds them cannot grow without moving the others.
    pub fn set_door(&mut self, index: usize, door: Door) -> bool {
        let Some(at) = self.door_slots().nth(index) else { return false };
        let words = [door.dest_room, door.x, door.y, door.dest_x, door.dest_y];
        for (i, w) in words.iter().enumerate() {
            self.head[at + i * 2..at + i * 2 + 2].copy_from_slice(&w.to_le_bytes());
        }
        true
    }

    /// Resizes the map, keeping the top-left cells and filling new ones with
    /// the empty tile.
    pub fn resize(&mut self, width: usize, height: usize) {
        // Block records address cells by index, which depends on the width.
        let blocks = self.blocks();
        let mut tiles = vec![0u16; width * height];
        for y in 0..height.min(self.height) {
            for x in 0..width.min(self.width) {
                tiles[y * width + x] = self.tile(x, y);
            }
        }
        self.tiles = tiles;
        self.width = width;
        self.height = height;
        if !blocks.is_empty() {
            self.set_blocks(&blocks);
        }
    }

    /// The uncompressed map blob: two tile planes, then the object list.
    pub fn blob(&self) -> Vec<u8> {
        let mut blob = Vec::with_capacity(self.tiles.len() * 2 + self.objects.len() * 6 + 2);
        blob.extend(self.tiles.iter().map(|&t| t as u8));
        blob.extend(self.tiles.iter().map(|&t| (t >> 8) as u8));
        for o in &self.objects {
            blob.extend_from_slice(&[o.variant, o.id]);
            blob.extend_from_slice(&o.x.to_le_bytes());
            blob.extend_from_slice(&o.y.to_le_bytes());
        }
        blob.extend_from_slice(&[0xFF, 0xFF]);
        blob.extend_from_slice(&self.tail);
        blob
    }

    /// The whole room block as it is written to the ROM.
    pub fn encode(&self) -> Result<Vec<u8>, RoomError> {
        if self.tiles.len() != self.width * self.height || self.width > 0xFFFF || self.height > 0xFFFF {
            return Err(RoomError::BadDimensions);
        }
        let blob = self.blob();
        if blob.len() > compression::MAX_OUTPUT {
            return Err(RoomError::BadDimensions);
        }
        let packed = compression::compress(&blob);
        match compression::decompress(&packed) {
            Ok((back, used)) if back == blob && used == packed.len() => {}
            _ => return Err(RoomError::RoundTrip),
        }
        let mut block = self.head.clone();
        block[self.size_at..self.size_at + 2].copy_from_slice(&(self.width as u16).to_le_bytes());
        block[self.size_at + 2..self.size_at + 4].copy_from_slice(&(self.height as u16).to_le_bytes());
        block.extend_from_slice(&packed);
        Ok(block)
    }

    /// Writes the room into `rom`.
    ///
    /// The block is written over the original when it fits. Otherwise it
    /// moves to free space, the room pointer follows it, and the old block
    /// is released. `self` and `tables` are updated to the new location.
    pub fn save(&mut self, rom: &mut Rom, tables: &mut Tables) -> Result<SaveReport, RoomError> {
        let block = self.encode()?;
        let old = (self.offset, self.stored_len);
        let moved = block.len() > self.stored_len;
        if moved {
            // The old block is free too, but data is never moved onto itself:
            // a failed write must leave the original intact.
            let target = find_free(rom.data(), block.len())
                .ok_or(RoomError::NoFreeSpace { needed: block.len() })?;
            rom.write(target, &block);
            rom.write_long_pointer(tables.room_table + self.index * 4, target);
            let shared = tables
                .room_offsets
                .iter()
                .enumerate()
                .any(|(i, &offset)| i != self.index && offset == old.0);
            if !shared {
                rom.write(old.0, &vec![0xFF; old.1]);
            }
            tables.room_offsets[self.index] = target;
            self.offset = target;
        } else {
            rom.write(self.offset, &block);
            let slack = self.stored_len - block.len();
            rom.write(self.offset + block.len(), &vec![0xFF; slack]);
        }
        self.stored_len = block.len();
        rom.fix_checksum();
        Ok(SaveReport { offset: self.offset, len: block.len(), previous_len: old.1, moved })
    }
}

/// What [`Room::save`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveReport {
    pub offset: usize,
    pub len: usize,
    pub previous_len: usize,
    /// True when the room no longer fitted and was moved to free space.
    pub moved: bool,
}

/// The run of `FF` that pads the end of each bank, as `(start, len)`, guard
/// bytes excluded. Only this padding is treated as free: a run of `FF` inside
/// a bank can be data (blank graphics, tables).
fn free_runs(data: &[u8]) -> impl Iterator<Item = (usize, usize)> + '_ {
    data.chunks(0x1_0000).enumerate().filter_map(|(bank, bytes)| {
        let used = bytes.iter().rposition(|&b| b != 0xFF).map_or(0, |last| last + 1);
        let run = bytes.len() - used;
        (run >= MIN_FREE_RUN).then(|| (bank * 0x1_0000 + used + FREE_GUARD, run - FREE_GUARD))
    })
}

/// Finds `needed` free bytes, in the smallest bank padding that fits.
fn find_free(data: &[u8], needed: usize) -> Option<usize> {
    free_runs(data).filter(|&(_, len)| len >= needed).min_by_key(|&(_, len)| len).map(|(start, _)| start)
}

/// Total free bytes [`Room::save`] could move rooms into.
pub fn free_space(rom: &Rom) -> usize {
    free_runs(rom.data()).map(|(_, len)| len).sum()
}

/// A stage: a run of consecutive rooms in the pointer table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub index: usize,
    pub name: String,
    pub rooms: std::ops::Range<usize>,
}

/// The pointer tables of bank `FF`.
#[derive(Debug, Clone)]
pub struct Tables {
    /// File offset of the room pointer table (4 bytes per room).
    pub room_table: usize,
    /// File offset of the resource pointer table (3 bytes per entry).
    pub gfx_table: usize,
    pub gfx_count: usize,
    pub room_offsets: Vec<usize>,
    pub levels: Vec<Level>,
}

/// The four stages of Spring Breeze are the first four levels of the table.
pub const SPRING_BREEZE_LEVELS: std::ops::Range<usize> = 0..4;
const SPRING_BREEZE_NAMES: [&str; 4] = ["Green Greens", "Float Islands", "Bubbly Clouds", "Mt. Dedede"];
const LEVEL_COUNT: usize = 60;

impl Tables {
    pub fn load(rom: &Rom) -> Self {
        let directory = |entry: usize| MASTER_TABLE + rom.u16(MASTER_TABLE + entry * 2) as usize;
        let room_table = directory(0);
        let gfx_table = directory(1);
        let gfx_end = directory(2);
        let room_count = (gfx_table - room_table) / 4;
        let room_offsets =
            (0..room_count).map(|i| rom.long_pointer(room_table + i * 4).unwrap_or(usize::MAX)).collect();
        let level_table = directory(6);
        let first_room = |level: usize| {
            let entry = MASTER_TABLE + rom.u16(level_table + level * 2) as usize;
            (entry.saturating_sub(room_table) / 4).min(room_count)
        };
        let levels = (0..LEVEL_COUNT)
            .map(|index| {
                let start = first_room(index);
                let end = if index + 1 < LEVEL_COUNT { first_room(index + 1) } else { room_count };
                let name = match SPRING_BREEZE_NAMES.get(index) {
                    Some(name) => (*name).to_string(),
                    None => format!("Level {index}"),
                };
                Level { index, name, rooms: start..end.max(start) }
            })
            .collect();
        Self { room_table, gfx_table, gfx_count: (gfx_end - gfx_table) / 3, room_offsets, levels }
    }

    pub fn room_count(&self) -> usize {
        self.room_offsets.len()
    }

    /// File offset of resource `index` of the graphics/palette table.
    pub fn resource(&self, rom: &Rom, index: usize) -> Option<usize> {
        (index > 0 && index < self.gfx_count).then(|| rom.long_pointer(self.gfx_table + index * 3)).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom() -> Option<Rom> {
        Rom::locate().map(|path| Rom::load(&path).expect("ROM loads"))
    }

    #[test]
    fn green_greens_room_1() {
        let Some(rom) = rom() else { return };
        let tables = Tables::load(&rom);
        assert_eq!(tables.room_count(), 319);
        assert_eq!(tables.levels[0].rooms, 0..6);
        assert_eq!(tables.levels[3].rooms, 24..27);
        let room = Room::load(&rom, &tables, 1).unwrap();
        assert_eq!(room.offset, 0x30_20EF);
        assert_eq!((room.width, room.height), (100, 8));
        assert_eq!(room.music(), 0x0F);
        assert_eq!(room.tile(15, 0), 0x0A0);
        assert_eq!(room.tile(0, 7), 0x00C);
        assert_eq!(room.objects.len(), 8);
        assert_eq!(room.objects[0], Object { variant: 0, id: 0x72, x: 0x18C, y: 0x84 });
        assert_eq!(room.objects[2], Object { variant: 0, id: 0x1A, x: 0x22D, y: 0x74 });
        let doors = room.doors();
        assert_eq!(doors.len(), 2);
        assert_eq!(doors[0], Door { dest_room: 2, x: 0x180, y: 0x78, dest_x: 0x9C, dest_y: 0x6C });
        assert!(!doors[1].has_trigger());
        assert_eq!(room.stored_len, 0x30_225F - 0x30_20EF);
    }

    #[test]
    fn every_room_round_trips() {
        let Some(rom) = rom() else { return };
        let tables = Tables::load(&rom);
        let mut loaded = 0;
        for index in 0..tables.room_count() {
            let Ok(room) = Room::load(&rom, &tables, index) else { continue };
            loaded += 1;
            let block = room.encode().unwrap();
            // Re-parse the encoded block from a scratch image.
            let mut copy = rom.clone();
            let at = 0x2F_E9A0;
            copy.write(at, &block);
            copy.write_long_pointer(tables.room_table + index * 4, at);
            let moved = Room::load(&copy, &Tables::load(&copy), index).unwrap();
            assert_eq!(moved.tiles, room.tiles, "room {index}");
            assert_eq!(moved.objects, room.objects, "room {index}");
            assert_eq!(moved.head(), room.head(), "room {index}");
        }
        assert!(loaded >= 300, "only {loaded} rooms loaded");
    }

    #[test]
    fn saving_unchanged_rooms_keeps_them_in_place() {
        let Some(original) = rom() else { return };
        let mut rom = original.clone();
        let mut tables = Tables::load(&rom);
        for index in tables.levels[0].rooms.start..tables.levels[3].rooms.end {
            let mut room = Room::load(&rom, &tables, index).unwrap();
            let report = room.save(&mut rom, &mut tables).unwrap();
            assert!(!report.moved, "room {index}: {} > {}", report.len, report.previous_len);
        }
    }

    #[test]
    fn a_grown_room_moves_and_reloads() {
        let Some(original) = rom() else { return };
        let mut rom = original.clone();
        let mut tables = Tables::load(&rom);
        let mut room = Room::load(&rom, &tables, 1).unwrap();
        // Noise does not compress: the block must outgrow its slot.
        let mut seed = 7u32;
        for tile in room.tiles.iter_mut() {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            *tile = (seed >> 16) as u16 % 300;
        }
        room.objects.push(Object { variant: 0, id: 1, x: 100, y: 100 });
        let report = room.save(&mut rom, &mut tables).unwrap();
        assert!(report.moved);
        let back = Room::load(&rom, &Tables::load(&rom), 1).unwrap();
        assert_eq!(back.tiles, room.tiles);
        assert_eq!(back.objects, room.objects);
        // The neighbours are untouched.
        for index in [0, 2, 17] {
            assert_eq!(
                Room::load(&rom, &tables, index).unwrap(),
                Room::load(&original, &Tables::load(&original), index).unwrap()
            );
        }
    }

    #[test]
    fn breakable_blocks_round_trip() {
        let Some(original) = rom() else { return };
        let mut rom = original.clone();
        let mut tables = Tables::load(&rom);
        // Room 3: four star blocks in row 4, their caps in row 3.
        let stars = Room::load(&rom, &tables, 3).unwrap();
        let blocks = stars.blocks();
        assert_eq!(blocks.len(), 8);
        assert_eq!(blocks[0], Block { x: 19, y: 3, after: 0, extra: 0 });
        assert_eq!(blocks[4], Block { x: 19, y: 4, after: 0xAC, extra: 0x57 });
        // Room 1 has none; add one and reload it from the image.
        let mut room = Room::load(&rom, &tables, 1).unwrap();
        assert!(room.blocks().is_empty());
        let doors = room.doors();
        room.set_tile(3, 5, 3);
        room.set_blocks(&[Block { x: 3, y: 5, after: 0, extra: 0 }]);
        room.save(&mut rom, &mut tables).unwrap();
        let back = Room::load(&rom, &tables, 1).unwrap();
        assert_eq!(back.blocks(), [Block { x: 3, y: 5, after: 0, extra: 0 }]);
        assert_eq!(back.doors(), doors);
        assert_eq!(back.tiles, room.tiles);
        assert_eq!((back.width, back.height), (100, 8));
        // Syncing a room that already lists its blocks changes nothing.
        let tiles = crate::gfx::Tileset::for_room(&original, &Tables::load(&original), &stars);
        let mut synced = stars.clone();
        assert!(!synced.sync_blocks(|t| tiles.is_breakable(t), &stars.tiles));
        // A new star block gets a record; painting it away drops it again.
        synced.set_tile(2, 2, 0x56);
        assert!(synced.sync_blocks(|t| tiles.is_breakable(t), &stars.tiles));
        assert!(synced.blocks().contains(&Block { x: 2, y: 2, after: 0, extra: 0 }));
        let with_block = synced.tiles.clone();
        synced.set_tile(2, 2, 0);
        assert!(synced.sync_blocks(|t| tiles.is_breakable(t), &with_block));
        assert_eq!(synced.blocks(), blocks);
        // Writing a room's own list back changes nothing.
        let mut same = stars.clone();
        same.set_blocks(&blocks);
        assert_eq!(same, stars);
    }

    #[test]
    fn resize_keeps_the_top_left() {
        let Some(rom) = rom() else { return };
        let tables = Tables::load(&rom);
        let mut room = Room::load(&rom, &tables, 1).unwrap();
        let corner = room.tile(15, 0);
        room.resize(50, 10);
        assert_eq!(room.tiles.len(), 500);
        assert_eq!(room.tile(15, 0), corner);
        assert_eq!(room.tile(49, 9), 0);
    }
}
