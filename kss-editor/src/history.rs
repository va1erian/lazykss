//! Undoable edits of one room.
//!
//! A [`Change`] knows how to apply itself to a [`Room`] and how to take
//! itself back. The editor stores groups of changes (a drag stroke is one
//! group); undoing a group reverts its changes in reverse order.

use kss_core::{Block, Door, Object, Room};

/// One cell of a [`Change::Tiles`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileChange {
    /// Row-major index into `Room::tiles`.
    pub index: usize,
    pub old: u16,
    pub new: u16,
}

/// The tile map of a room and its breakable-cell list, for resizes. The
/// list goes with the map: its records address cells by index, which depends
/// on the width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapSnapshot {
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<u16>,
    pub blocks: Vec<Block>,
}

impl MapSnapshot {
    pub fn of(room: &Room) -> Self {
        Self {
            width: room.width,
            height: room.height,
            tiles: room.tiles.clone(),
            blocks: room.blocks(),
        }
    }

    /// The tile at a cell, `None` outside the map.
    pub fn tile(&self, x: usize, y: usize) -> Option<u16> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.tiles.get(y * self.width + x).copied()
    }

    fn restore(&self, room: &mut Room) {
        room.width = self.width;
        room.height = self.height;
        room.tiles = self.tiles.clone();
        room.set_blocks(&self.blocks);
    }
}

/// Sets one tile of `room`. An index past the map is left alone: the change
/// was recorded for another map size, and there is no cell to write.
fn set_cell(room: &mut Room, index: usize, tile: u16) {
    if let Some(slot) = room.tiles.get_mut(index) {
        *slot = tile;
    }
}

/// A single edit, already applied or about to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Tiles(Vec<TileChange>),
    AddObject {
        index: usize,
        object: Object,
    },
    RemoveObject {
        index: usize,
        object: Object,
    },
    Object {
        index: usize,
        old: Object,
        new: Object,
    },
    Door {
        index: usize,
        old: Door,
        new: Door,
    },
    InsertDoor {
        index: usize,
        door: Door,
    },
    RemoveDoor {
        index: usize,
        door: Door,
    },
    Music {
        old: u8,
        new: u8,
    },
    Resize {
        old: Box<MapSnapshot>,
        new: Box<MapSnapshot>,
    },
}

impl Change {
    /// Whether the change adds or removes an object, shifting the indices
    /// of the ones after it.
    pub fn reorders_objects(&self) -> bool {
        matches!(self, Change::AddObject { .. } | Change::RemoveObject { .. })
    }

    /// Whether the change adds or removes a door.
    pub fn reorders_doors(&self) -> bool {
        matches!(self, Change::InsertDoor { .. } | Change::RemoveDoor { .. })
    }

    /// Whether the change alters what the room image shows.
    pub fn alters_image(&self) -> bool {
        matches!(self, Change::Tiles(_) | Change::Resize { .. })
    }

    pub fn apply(&self, room: &mut Room) {
        match self {
            Change::Tiles(cells) => {
                for c in cells {
                    set_cell(room, c.index, c.new);
                }
            }
            Change::AddObject { index, object } => room
                .objects
                .insert((*index).min(room.objects.len()), *object),
            Change::RemoveObject { index, .. } => {
                if *index < room.objects.len() {
                    room.objects.remove(*index);
                }
            }
            Change::Object { index, new, .. } => {
                if let Some(slot) = room.objects.get_mut(*index) {
                    *slot = *new;
                }
            }
            Change::Door { index, new, .. } => {
                room.set_door(*index, *new);
            }
            Change::InsertDoor { index, door } => {
                room.insert_door(*index, *door);
            }
            Change::RemoveDoor { index, .. } => {
                room.remove_door(*index);
            }
            Change::Music { new, .. } => room.set_music(*new),
            Change::Resize { new, .. } => new.restore(room),
        }
    }

    pub fn revert(&self, room: &mut Room) {
        match self {
            Change::Tiles(cells) => {
                for c in cells.iter().rev() {
                    set_cell(room, c.index, c.old);
                }
            }
            Change::AddObject { index, .. } => {
                if *index < room.objects.len() {
                    room.objects.remove(*index);
                }
            }
            Change::RemoveObject { index, object } => {
                room.objects
                    .insert((*index).min(room.objects.len()), *object);
            }
            Change::Object { index, old, .. } => {
                if let Some(slot) = room.objects.get_mut(*index) {
                    *slot = *old;
                }
            }
            Change::Door { index, old, .. } => {
                room.set_door(*index, *old);
            }
            Change::InsertDoor { index, .. } => {
                room.remove_door(*index);
            }
            Change::RemoveDoor { index, door } => {
                room.insert_door(*index, *door);
            }
            Change::Music { old, .. } => room.set_music(*old),
            Change::Resize { old, .. } => old.restore(room),
        }
    }

    /// Folds `next` into `self` when both are the same kind of edit on the
    /// same thing, so a drag does not leave one entry per mouse move.
    /// Returns `next` back when they do not merge.
    pub fn merge(&mut self, next: Change) -> Option<Change> {
        match (self, next) {
            (Change::Tiles(a), Change::Tiles(b)) => {
                a.extend(b);
                None
            }
            (
                Change::Object { index, new, .. },
                Change::Object {
                    index: i2, new: n2, ..
                },
            ) if *index == i2 => {
                *new = n2;
                None
            }
            (
                Change::Door { index, new, .. },
                Change::Door {
                    index: i2, new: n2, ..
                },
            ) if *index == i2 => {
                *new = n2;
                None
            }
            (_, next) => Some(next),
        }
    }
}

/// An undo step: the changes of one gesture, oldest first.
pub type Entry = Vec<Change>;
