//! Which rooms are loaded, which one is current, and the accessors of the
//! current room.

use std::collections::BTreeMap;

use kss_core::{Rom, Room, Tables, Tileset};

use super::{Editor, LoadedRoom};
use crate::history::MapSnapshot;

/// Reads room `index` from the ROM, unmodified and with an empty history.
pub(super) fn load_room(rom: &Rom, tables: &Tables, index: usize) -> Result<LoadedRoom, String> {
    let room = Room::load(rom, tables, index).map_err(|e| e.to_string())?;
    let tileset = Tileset::for_room(rom, tables, &room);
    Ok(LoadedRoom {
        saved: MapSnapshot::of(&room),
        room,
        tileset,
        revision: 0,
        undo: Vec::new(),
        redo: Vec::new(),
        dirty: false,
    })
}

/// The loaded rooms. The current one is held apart from the others, so
/// there is always a current room and no lookup of it can fail.
pub(super) struct Rooms {
    current: usize,
    active: LoadedRoom,
    /// Every other loaded room; never holds `current`.
    others: BTreeMap<usize, LoadedRoom>,
}

impl Rooms {
    pub(super) fn new(current: usize, active: LoadedRoom) -> Self {
        Self {
            current,
            active,
            others: BTreeMap::new(),
        }
    }

    pub(super) fn current(&self) -> usize {
        self.current
    }

    pub(super) fn active(&self) -> &LoadedRoom {
        &self.active
    }

    pub(super) fn active_mut(&mut self) -> &mut LoadedRoom {
        &mut self.active
    }

    pub(super) fn get(&self, index: usize) -> Option<&LoadedRoom> {
        if index == self.current {
            Some(&self.active)
        } else {
            self.others.get(&index)
        }
    }

    pub(super) fn get_mut(&mut self, index: usize) -> Option<&mut LoadedRoom> {
        if index == self.current {
            Some(&mut self.active)
        } else {
            self.others.get_mut(&index)
        }
    }

    /// Adds a room that is not current. The current room is never replaced
    /// this way: it would lose its history.
    pub(super) fn insert(&mut self, index: usize, room: LoadedRoom) {
        if index != self.current {
            self.others.insert(index, room);
        }
    }

    /// Makes `index` current, loading it with `load` when it is not loaded
    /// yet. On failure the current room stays as it was.
    pub(super) fn switch_to(
        &mut self,
        index: usize,
        load: impl FnOnce() -> Result<LoadedRoom, String>,
    ) -> Result<(), String> {
        if index == self.current {
            return Ok(());
        }
        let next = match self.others.remove(&index) {
            Some(room) => room,
            None => load()?,
        };
        let previous = std::mem::replace(&mut self.active, next);
        self.others.insert(self.current, previous);
        self.current = index;
        Ok(())
    }

    /// Every loaded room, in index order.
    pub(super) fn iter(&self) -> impl Iterator<Item = (usize, &LoadedRoom)> + '_ {
        let current = (self.current, &self.active);
        let before = self.others.range(..self.current);
        let after = self.others.range(self.current..);
        before
            .map(|(&i, r)| (i, r))
            .chain(std::iter::once(current))
            .chain(after.map(|(&i, r)| (i, r)))
    }
}

impl Editor {
    /// Makes `index` the current room, loading it if needed.
    pub fn select_room(&mut self, index: usize) -> Result<(), String> {
        self.end_group();
        let previous = self.rooms.current();
        let (rom, tables) = (&self.rom, &self.tables);
        self.rooms
            .switch_to(index, || load_room(rom, tables, index))?;
        if previous != index {
            self.selected_object = None;
            self.selected_door = None;
        }
        Ok(())
    }

    pub fn current_index(&self) -> usize {
        self.rooms.current()
    }

    pub fn loaded(&self, index: usize) -> Option<&LoadedRoom> {
        self.rooms.get(index)
    }

    pub fn room(&self) -> &Room {
        &self.rooms.active().room
    }

    pub fn tileset(&self) -> &Tileset {
        &self.rooms.active().tileset
    }

    /// Changes whenever the current room image must be re-rendered.
    pub fn revision(&self) -> u64 {
        self.rooms.active().revision
    }

    pub fn is_dirty(&self, index: usize) -> bool {
        self.rooms.get(index).is_some_and(|r| r.dirty)
    }

    pub fn dirty_rooms(&self) -> Vec<usize> {
        self.rooms
            .iter()
            .filter(|(_, r)| r.dirty)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn any_dirty(&self) -> bool {
        self.rooms.iter().any(|(_, r)| r.dirty)
    }

    pub fn can_undo(&self) -> bool {
        !self.rooms.active().undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.rooms.active().redo.is_empty()
    }
}
