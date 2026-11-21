//! Placing, moving and naming the objects of the current room, and the
//! treasure chests of the cave.

use kss_core::names::{TREASURE_CHEST, WARP_STAR};
use kss_core::{CELL, Object};

use super::{ChestSpot, Editor};
use crate::history::Change;

/// A coordinate clamped to a room `cells` cells long: from 0 to its last
/// pixel, and to what an object position (a u16) holds.
fn clamp_axis(value: i32, cells: usize) -> u16 {
    let last = cells.saturating_mul(CELL).saturating_sub(1);
    let last = u16::try_from(last).unwrap_or(u16::MAX);
    u16::try_from(value.clamp(0, i32::from(last))).unwrap_or(last)
}

impl Editor {
    /// A room-pixel position clamped to the current room.
    pub(super) fn clamp_position(&self, x: i32, y: i32) -> (u16, u16) {
        let room = self.room();
        (clamp_axis(x, room.width), clamp_axis(y, room.height))
    }

    /// An object's name as the lists and the map show it: a chest names its
    /// treasure, a warp star where it goes.
    pub fn object_label(&self, o: &Object) -> String {
        match o.id {
            TREASURE_CHEST => match kss_core::treasure::treasure(o.variant) {
                Some(t) => format!("Treasure {}: {}", o.variant, t.name),
                None => format!("Treasure chest {} (no such treasure)", o.variant),
            },
            WARP_STAR => match o
                .warp_door()
                .and_then(|i| self.room().doors().get(i).copied())
            {
                Some(door) => format!("Warp Star \u{2192} {}", self.destination_label(&door)),
                None => "Warp Star (no door record)".to_string(),
            },
            id => kss_core::names::object_name(id).unwrap_or("").to_string(),
        }
    }

    /// The object nearest to a room-pixel position within `radius` pixels.
    /// Later objects win ties (they are drawn on top).
    pub fn object_at(&self, x: i32, y: i32, radius: i32) -> Option<usize> {
        // In i64, so a position far outside the room cannot overflow.
        let (x, y, radius) = (i64::from(x), i64::from(y), i64::from(radius));
        let mut best: Option<(usize, i64)> = None;
        for (i, o) in self.room().objects.iter().enumerate() {
            let (dx, dy) = ((i64::from(o.x) - x).abs(), (i64::from(o.y) - y).abs());
            if dx <= radius && dy <= radius {
                let d = dx.max(dy);
                if best.is_none_or(|(_, bd)| d <= bd) {
                    best = Some((i, d));
                }
            }
        }
        best.map(|(i, _)| i)
    }

    /// Places an object of the current `new_object` kind and selects it.
    pub fn add_object(&mut self, x: i32, y: i32) -> usize {
        let (px, py) = self.clamp_position(x, y);
        let object = Object {
            id: self.new_object.0,
            variant: self.new_object.1,
            x: px,
            y: py,
        };
        let index = self.room().objects.len();
        self.perform(Change::AddObject { index, object });
        self.selected_object = Some(index);
        index
    }

    pub fn remove_object(&mut self, index: usize) -> bool {
        let Some(object) = self.room().objects.get(index).copied() else {
            return false;
        };
        self.perform(Change::RemoveObject { index, object });
        self.selected_object = None;
        true
    }

    /// Replaces object `index`. A no-op when nothing differs.
    pub fn set_object(&mut self, index: usize, new: Object) -> bool {
        let Some(old) = self.room().objects.get(index).copied() else {
            return false;
        };
        let (x, y) = self.clamp_position(i32::from(new.x), i32::from(new.y));
        let new = Object { x, y, ..new };
        if old == new {
            return false;
        }
        self.perform(Change::Object { index, old, new });
        true
    }

    /// Moves object `index` to a room-pixel position (clamped to the room).
    pub fn move_object(&mut self, index: usize, x: i32, y: i32) -> bool {
        let Some(old) = self.room().objects.get(index).copied() else {
            return false;
        };
        let (x, y) = self.clamp_position(x, y);
        self.set_object(index, Object { x, y, ..old })
    }

    pub fn nudge_object(&mut self, index: usize, dx: i32, dy: i32) -> bool {
        let Some(old) = self.room().objects.get(index).copied() else {
            return false;
        };
        let x = i32::from(old.x).saturating_add(dx);
        let y = i32::from(old.y).saturating_add(dy);
        self.move_object(index, x, y)
    }

    /// The warp stars of the current room that use door `index`.
    pub fn warps_of(&self, index: usize) -> Vec<usize> {
        self.room()
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| o.warp_door() == Some(index))
            .map(|(i, _)| i)
            .collect()
    }

    /// Every treasure chest in the cave's rooms, edits included, by
    /// treasure number then room.
    pub fn chests(&self) -> Vec<ChestSpot> {
        let mut spots = Vec::new();
        for room in self.cave_rooms() {
            let Some(lr) = self.rooms.get(room) else {
                continue;
            };
            for (object, o) in lr.room.objects.iter().enumerate() {
                if o.id == TREASURE_CHEST {
                    spots.push(ChestSpot {
                        treasure: o.variant,
                        room,
                        object,
                        x: o.x,
                        y: o.y,
                    });
                }
            }
        }
        spots.sort_by_key(|s| (s.treasure, s.room, s.object));
        spots
    }
}
