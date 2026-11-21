//! The doors of the current room, their door stars and the warp stars that
//! use them.

use kss_core::names::{DOOR_STAR, WARP_STAR};
use kss_core::room::{Destination, MAX_DOORS};
use kss_core::{CELL, Door, Object};

use super::Editor;
use crate::history::Change;

/// The collision attribute of a door tile.
const DOOR_ATTRIBUTE: u8 = 0x0F;

/// The side of a door's trigger cell, in pixels.
const CELL_PX: u16 = CELL as u16;

impl Editor {
    /// Where a door of the current room leads.
    pub fn destination(&self, door: &Door) -> Option<Destination> {
        self.level().map(|l| door.destination(l))
    }

    /// Where a door of the current room leads, in words.
    pub fn destination_label(&self, door: &Door) -> String {
        match self.destination(door) {
            Some(Destination::Room(r)) if r < self.room_count() => format!("room {r}"),
            Some(Destination::Room(r)) => format!("room {r} (missing)"),
            Some(Destination::LevelExit) => "stage exit".to_string(),
            Some(Destination::Special(v)) => format!("special {v:04X}"),
            None => format!("#{}", door.dest_room),
        }
    }

    /// False when a door's trigger cell holds no door tile (metatile
    /// attribute `0F`): Kirby cannot enter it there. `8F` is accepted too:
    /// Aqualiss's original doors sit on it (untested in the game).
    /// Some original doors start under a breakable block and are only
    /// entered once it is broken. A door without a trigger is never
    /// flagged.
    pub fn door_tile_present(&self, door: &Door) -> bool {
        if !door.has_trigger() {
            return true;
        }
        let cell = |v: u16| usize::from(v / CELL_PX);
        let tile = self.room().tile(cell(door.x), cell(door.y));
        self.tileset().attribute(tile) & 0x7F == DOOR_ATTRIBUTE
    }

    /// The door whose trigger box (the cell whose top-left corner is its
    /// position) contains a room-pixel position. Doors without a trigger
    /// have no box.
    pub fn door_at(&self, x: i32, y: i32) -> Option<usize> {
        // The position is the top-left corner of the door's cell: the
        // game's door stars sit 12 pixels right of and below it.
        let cell = i64::from(CELL_PX);
        let (x, y) = (i64::from(x), i64::from(y));
        self.room().doors().iter().position(|d| {
            let (dx, dy) = (x - i64::from(d.x), y - i64::from(d.y));
            d.has_trigger() && (0..cell).contains(&dx) && (0..cell).contains(&dy)
        })
    }

    pub fn set_door(&mut self, index: usize, new: Door) -> bool {
        let doors = self.room().doors();
        let Some(old) = doors.get(index).copied() else {
            return false;
        };
        // FFFF ends the list: it cannot be a destination.
        if old == new || new.dest_room == 0xFFFF {
            return false;
        }
        self.perform(Change::Door { index, old, new });
        true
    }

    /// Adds a door whose trigger is the cell under a room-pixel position,
    /// with the invisible door star the game's own doors have at the cell's
    /// centre. It leads back into this room at the same place until edited.
    /// Selects it and returns its index.
    pub fn add_door(&mut self, x: i32, y: i32) -> Option<usize> {
        let (px, py) = self.clamp_position(x, y);
        let (cx, cy) = (px - px % CELL_PX, py - py % CELL_PX);
        let half = CELL_PX / 2;
        let (centre_x, centre_y) = (cx.saturating_add(half), cy.saturating_add(half));
        let here = self
            .level()
            .and_then(|l| Door::relative_to(l, self.current_index()))?;
        let door = Door {
            dest_room: here,
            x: cx,
            y: cy,
            dest_x: centre_x,
            dest_y: centre_y,
        };
        let star = Object {
            variant: 0,
            id: DOOR_STAR,
            x: centre_x,
            y: centre_y,
        };
        self.insert_door_with(door, star)
    }

    /// Adds a warp star at a room-pixel position together with the door
    /// record it uses (one without a trigger area). Selects the door.
    pub fn add_warp(&mut self, x: i32, y: i32) -> Option<usize> {
        let (px, py) = self.clamp_position(x, y);
        let here = self
            .level()
            .and_then(|l| Door::relative_to(l, self.current_index()))?;
        let door = Door {
            dest_room: here,
            x: Door::NO_TRIGGER,
            y: Door::NO_TRIGGER,
            dest_x: px,
            dest_y: py,
        };
        let star = Object {
            variant: u8::try_from(self.room().doors().len()).unwrap_or(u8::MAX),
            id: WARP_STAR,
            x: px,
            y: py,
        };
        self.insert_door_with(door, star)
    }

    /// Appends `door` and the object that marks it, in one undo step.
    fn insert_door_with(&mut self, door: Door, marker: Object) -> Option<usize> {
        let index = self.room().doors().len();
        if index >= MAX_DOORS {
            return None;
        }
        self.as_one_step(|e| {
            e.perform(Change::InsertDoor { index, door });
            let at = e.room().objects.len();
            e.perform(Change::AddObject {
                index: at,
                object: marker,
            });
        });
        self.selected_door = Some(index);
        self.selected_object = None;
        Some(index)
    }

    /// Removes door `index`, the door star on its trigger and the warp stars
    /// that use it, and renumbers the warp stars of later doors: one undo
    /// step.
    pub fn remove_door(&mut self, index: usize) -> bool {
        let Some(door) = self.room().doors().get(index).copied() else {
            return false;
        };
        self.as_one_step(|e| {
            e.perform(Change::RemoveDoor { index, door });
            // Highest index first, so the indices still to remove stay valid.
            for i in e.objects_of_removed_door(index, door).into_iter().rev() {
                if let Some(object) = e.room().objects.get(i).copied() {
                    e.perform(Change::RemoveObject { index: i, object });
                }
            }
            e.renumber_warps_after(index);
        });
        self.selected_door = None;
        self.selected_object = None;
        true
    }

    /// The objects that go with door `index`, already removed from the
    /// list: its warp stars and the door star on its trigger cell, in
    /// ascending order.
    fn objects_of_removed_door(&self, index: usize, door: Door) -> Vec<usize> {
        let half = CELL_PX / 2;
        // Wrapping: a door from the ROM may sit anywhere, even at the end
        // of the coordinate range.
        let star_at = (door.x.wrapping_add(half), door.y.wrapping_add(half));
        // When a remaining door has its trigger on the same cell, one star
        // stays for it.
        let shared = self
            .room()
            .doors()
            .iter()
            .any(|d| (d.x, d.y) == (door.x, door.y));
        let stars = self
            .room()
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| door.has_trigger() && o.id == DOOR_STAR && (o.x, o.y) == star_at)
            .map(|(i, _)| i)
            .skip(usize::from(shared));
        let mut doomed: Vec<usize> = self.warps_of(index).into_iter().chain(stars).collect();
        doomed.sort_unstable();
        doomed
    }

    /// After door `removed` is gone, points the warp stars of the doors
    /// after it at their new, one lower, index.
    fn renumber_warps_after(&mut self, removed: usize) {
        let later: Vec<(usize, Object)> = self
            .room()
            .objects
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, o)| o.warp_door().is_some_and(|d| d > removed))
            .collect();
        for (index, old) in later {
            // Its door index (the variant) is above `removed`, so at least 1.
            let new = Object {
                variant: old.variant.saturating_sub(1),
                ..old
            };
            self.perform(Change::Object { index, old, new });
        }
    }
}
