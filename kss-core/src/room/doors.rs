//! Section 0: the door list, and where doors lead.

use std::ops::Range;

use super::bytes::{offset_of, word};
use super::{Level, MAX_DOORS, Room, SECTIONS_OFFSET};

/// Size of one door record.
const DOOR_SIZE: usize = 10;
/// Offset in the head of the first door record, past section 0's length.
const DOORS_START: usize = SECTIONS_OFFSET + 2;
/// The word that ends the door list. No door can lead to it.
const TERMINATOR: u16 = 0xFFFF;
/// A flag two of the game's destinations carry (room 59 leads to "4008",
/// room 167 to "4007"): the game enters room `n` for `4000 | n` as for `n`. What the flag
/// changes is not established; it is kept when the room is changed.
const ROOM_FLAG: u16 = 0x4000;
/// Destinations from here up do not name a room.
const FIRST_SPECIAL: u16 = 0x8000;

/// A door record of section 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Door {
    /// The destination, counted from the first room of the level the door
    /// is in (see [`Door::destination`]); a few values are special.
    pub dest_room: u16,
    /// Trigger position in pixels. `FF00` on both axes marks an exit that is
    /// not entered by walking into it (a warp star, the end of a room).
    pub x: u16,
    pub y: u16,
    /// Where Kirby appears in the destination room.
    pub dest_x: u16,
    pub dest_y: u16,
}

/// Where a door leads, once its level is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// A room of the pointer table.
    Room(usize),
    /// `7FFF`: the goal door of a stage (its door star has variant 16).
    LevelExit,
    /// A value of `8000` and up; none occurs in the game.
    Special(u16),
}

impl Door {
    /// The `dest_room` of a stage's goal door.
    pub const LEVEL_EXIT: u16 = 0x7FFF;
    /// The position of a door that has no trigger area: a warp star's
    /// destination, or the end of a room.
    pub const NO_TRIGGER: u16 = 0xFF00;

    /// False for the exits that have no place in the room.
    pub fn has_trigger(&self) -> bool {
        self.x != Self::NO_TRIGGER || self.y != Self::NO_TRIGGER
    }

    /// The room this door leads to when it is in `level`. Destinations count
    /// from the level's first room: Float Islands' room 7 leads to "2",
    /// which is room 8.
    pub fn destination(&self, level: &Level) -> Destination {
        match self.dest_room {
            Self::LEVEL_EXIT => Destination::LevelExit,
            // A level starts at a table index, far below any overflow; a
            // saturated index names no room, as a bad one would.
            d if d < FIRST_SPECIAL => {
                let room = usize::from(d & !ROOM_FLAG);
                Destination::Room(level.rooms.start.saturating_add(room))
            }
            d => Destination::Special(d),
        }
    }

    /// The `dest_room` value that leads to `room` from a door of `level`,
    /// when that is expressible.
    pub fn relative_to(level: &Level, room: usize) -> Option<u16> {
        room.checked_sub(level.rooms.start)
            .and_then(|d| u16::try_from(d).ok())
            .filter(|&d| d < ROOM_FLAG)
    }

    /// True for a destination that names a room with the `4000` flag.
    pub fn has_room_flag(&self) -> bool {
        self.dest_room != Self::LEVEL_EXIT
            && self.dest_room & (FIRST_SPECIAL | ROOM_FLAG) == ROOM_FLAG
    }

    /// This door, leading to `room` of `level` instead, with the `4000`
    /// flag kept when it had one. `None` when `room` cannot be stored.
    pub fn retargeted(&self, level: &Level, room: usize) -> Option<Door> {
        let relative = Self::relative_to(level, room)?;
        let flag = if self.has_room_flag() { ROOM_FLAG } else { 0 };
        Some(Door {
            dest_room: relative | flag,
            ..*self
        })
    }

    fn words(&self) -> [u16; 5] {
        [self.dest_room, self.x, self.y, self.dest_x, self.dest_y]
    }

    /// Reads one record; `None` when it is short.
    fn from_record(record: &[u8]) -> Option<Self> {
        Some(Self {
            dest_room: word(record, 0)?,
            x: word(record, 2)?,
            y: word(record, 4)?,
            dest_x: word(record, 6)?,
            dest_y: word(record, 8)?,
        })
    }

    /// The record as it is stored in section 0.
    fn record(&self) -> impl Iterator<Item = u8> {
        self.words().into_iter().flat_map(u16::to_le_bytes)
    }
}

impl Room {
    /// The door records of section 0, up to the terminator.
    fn door_records(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.head
            .get(DOORS_START..self.doors_end)
            .unwrap_or_default()
            .chunks_exact(DOOR_SIZE)
            .take_while(|record| !record.starts_with(&TERMINATOR.to_le_bytes()))
    }

    /// Range in `head` of door `index`, when the room has that door.
    fn door_slot(&self, index: usize) -> Option<Range<usize>> {
        if index >= self.door_records().count() {
            return None;
        }
        let start = offset_of(DOORS_START, index, DOOR_SIZE)?;
        Some(start..start.checked_add(DOOR_SIZE)?)
    }

    pub fn doors(&self) -> Vec<Door> {
        self.door_records().filter_map(Door::from_record).collect()
    }

    /// Rewrites door `index` in place. A destination of `FFFF` is refused:
    /// it would read as the end of the list.
    pub fn set_door(&mut self, index: usize, door: Door) -> bool {
        if door.dest_room == TERMINATOR {
            return false;
        }
        let slot = self.door_slot(index);
        let Some(record) = slot.and_then(|slot| self.head.get_mut(slot)) else {
            return false;
        };
        for (dst, src) in record.iter_mut().zip(door.record()) {
            *dst = src;
        }
        true
    }

    /// Replaces the whole door list; the sections after it move. Doors with
    /// a destination of `FFFF` are left out (it is the terminator). Returns
    /// false, with the room unchanged, when the head's section words do not
    /// add up.
    pub fn set_doors(&mut self, doors: &[Door]) -> bool {
        let mut payload: Vec<u8> = doors
            .iter()
            .filter(|d| d.dest_room != TERMINATOR)
            .take(MAX_DOORS)
            .flat_map(Door::record)
            .collect();
        payload.extend_from_slice(&TERMINATOR.to_le_bytes());
        self.set_section(0, &payload).is_some()
    }

    /// Inserts a door at `index` (at most the number of doors). Warp stars
    /// address doors by index: the caller renumbers them.
    pub fn insert_door(&mut self, index: usize, door: Door) -> bool {
        let mut doors = self.doors();
        if index > doors.len() || doors.len() >= MAX_DOORS || door.dest_room == TERMINATOR {
            return false;
        }
        doors.insert(index, door);
        self.set_doors(&doors)
    }

    /// Removes door `index` and returns it.
    pub fn remove_door(&mut self, index: usize) -> Option<Door> {
        let mut doors = self.doors();
        if index >= doors.len() {
            return None;
        }
        let door = doors.remove(index);
        self.set_doors(&doors).then_some(door)
    }
}
