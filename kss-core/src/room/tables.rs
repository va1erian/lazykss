//! The pointer tables of bank `FF`: rooms, graphics resources and levels.

use super::bytes::{long_pointer, offset_of, word};
use crate::rom::Rom;

/// The directory of bank `FF`: words, relative to it, locating each table.
const MASTER_TABLE: usize = 0x3F_0000;
/// Directory entries of the tables read here.
const ROOM_ENTRY: usize = 0;
const GFX_ENTRY: usize = 1;
const GFX_END_ENTRY: usize = 2;
const LEVEL_ENTRY: usize = 6;
/// Bytes per entry of the room and resource pointer tables.
const ROOM_POINTER: usize = 4;
const GFX_POINTER: usize = 3;
const LEVEL_COUNT: usize = 60;

/// The four stages of Spring Breeze are the first four levels of the table.
pub const SPRING_BREEZE_LEVELS: std::ops::Range<usize> = 0..4;
/// The Great Cave Offensive is one level of 81 rooms (72-152): the level
/// that holds the treasure chests.
pub const GREAT_CAVE_OFFENSIVE: usize = 15;
/// The levels the editor lists by name, in order.
pub const NAMED_LEVELS: [usize; 5] = [0, 1, 2, 3, GREAT_CAVE_OFFENSIVE];

fn level_name(index: usize) -> Option<&'static str> {
    Some(match index {
        0 => "Green Greens",
        1 => "Float Islands",
        2 => "Bubbly Clouds",
        3 => "Mt. Dedede",
        GREAT_CAVE_OFFENSIVE => "The Great Cave Offensive",
        _ => return None,
    })
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

/// The file offset a bank `FF` word at `at` names.
fn in_bank_ff(data: &[u8], at: usize) -> Option<usize> {
    MASTER_TABLE.checked_add(usize::from(word(data, at)?))
}

/// The file offset of the table directory `entry` locates. A directory
/// outside the image reads as an empty table at its own address.
fn directory(data: &[u8], entry: usize) -> usize {
    offset_of(MASTER_TABLE, entry, 2)
        .and_then(|at| in_bank_ff(data, at))
        .unwrap_or(MASTER_TABLE)
}

impl Tables {
    pub fn load(rom: &Rom) -> Self {
        let data = rom.data();
        let room_table = directory(data, ROOM_ENTRY);
        let gfx_table = directory(data, GFX_ENTRY);
        let gfx_end = directory(data, GFX_END_ENTRY);
        // A hacked image can have the tables in any order: never underflow.
        let room_count = gfx_table.saturating_sub(room_table) / ROOM_POINTER;
        let room_offsets = (0..room_count)
            .map(|i| {
                offset_of(room_table, i, ROOM_POINTER)
                    .and_then(|at| long_pointer(data, at))
                    .unwrap_or(usize::MAX)
            })
            .collect();
        let level_table = directory(data, LEVEL_ENTRY);
        // The level table lists each level's first room; one that cannot be
        // read starts past the last room, so it is empty.
        let first_room = |level: usize| {
            offset_of(level_table, level, 2)
                .and_then(|at| in_bank_ff(data, at))
                .map_or(room_count, |entry| {
                    (entry.saturating_sub(room_table) / ROOM_POINTER).min(room_count)
                })
        };
        // A level ends where the next one starts; the last at the last room.
        let starts: Vec<usize> = (0..LEVEL_COUNT).map(first_room).collect();
        let ends = starts.iter().skip(1).copied().chain([room_count]);
        let levels = starts
            .iter()
            .zip(ends)
            .enumerate()
            .map(|(index, (&start, end))| Level {
                index,
                name: level_name(index).map_or_else(|| format!("Level {index}"), str::to_string),
                rooms: start..end.max(start),
            })
            .collect();
        Self {
            room_table,
            gfx_table,
            gfx_count: gfx_end.saturating_sub(gfx_table) / GFX_POINTER,
            room_offsets,
            levels,
        }
    }

    pub fn room_count(&self) -> usize {
        self.room_offsets.len()
    }

    /// The level whose range holds `room`.
    pub fn level_of(&self, room: usize) -> Option<&Level> {
        self.levels.iter().find(|l| l.rooms.contains(&room))
    }

    /// File offset of resource `index` of the graphics/palette table.
    pub fn resource(&self, rom: &Rom, index: usize) -> Option<usize> {
        if index == 0 || index >= self.gfx_count {
            return None;
        }
        long_pointer(rom.data(), offset_of(self.gfx_table, index, GFX_POINTER)?)
    }
}
