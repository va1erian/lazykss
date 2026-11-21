//! The side panels: the room browser and the objects, doors, treasures and
//! room tabs.

use kss_core::names::TREASURE_CHEST;
use kss_core::{Destination, Door, Object};

use super::super::tree::{self, Node};
use super::{DoorField, EditorApp, ObjField};
use crate::editor::Tool;

/// A number field's value as a byte. The fields are limited to their range,
/// but a typed value still passes through here, so it is clamped again.
fn to_u8(value: f64) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

/// A number field's value as a 16-bit word, clamped like [`to_u8`].
fn to_u16(value: f64) -> u16 {
    value.round().clamp(0.0, 65535.0) as u16
}

impl EditorApp {
    // ---- browser ----

    pub(super) fn browser_select(&mut self, id: usize) {
        if let Some(Node::Room(room)) = self.tree_nodes.get(id).copied() {
            self.load_room(room);
        }
    }

    pub(super) fn browser_toggle(&mut self, id: usize, open: bool) {
        let group = self.tree_nodes.get(id).copied().and_then(tree::group_index);
        if let Some(flag) = group.and_then(|g| self.expanded.get_mut(g)) {
            *flag = open;
        }
    }

    // ---- objects ----

    pub(super) fn select_object(&mut self, row: usize) {
        self.ed_mut(|e| {
            e.selected_object = Some(row);
            e.selected_door = None;
        });
        self.refresh_object_fields();
        self.h.map.get().invalidate();
    }

    pub(super) fn object_field(&mut self, field: ObjField, value: f64) {
        let value = to_u16(value);
        let changed = self
            .ed_mut(|e| {
                let i = e.selected_object?;
                let o = *e.room().objects.get(i)?;
                let new = match field {
                    // The byte fields' range is 0..=255: `as` keeps the value.
                    ObjField::Id => Object {
                        id: value as u8,
                        ..o
                    },
                    ObjField::Variant => Object {
                        variant: value as u8,
                        ..o
                    },
                    ObjField::X => Object { x: value, ..o },
                    ObjField::Y => Object { y: value, ..o },
                };
                Some(e.set_object(i, new))
            })
            .flatten()
            .unwrap_or(false);
        if changed {
            self.after_edit();
        }
    }

    pub(super) fn new_object_field(&mut self, field: ObjField, value: f64) {
        let value = to_u8(value);
        self.ed_mut(|e| match field {
            ObjField::Variant => e.new_object.1 = value,
            _ => e.new_object.0 = value,
        });
    }

    /// Adds an object in the middle of what is visible.
    pub(super) fn add_object_in_view(&mut self) {
        let pos = self.view_centre();
        self.ed_mut(|e| e.add_object(pos.0, pos.1));
        self.after_edit();
        self.select_object_row();
    }

    pub(super) fn delete_object(&mut self) {
        let removed = self
            .ed_mut(|e| e.selected_object.is_some_and(|i| e.remove_object(i)))
            .unwrap_or(false);
        if removed {
            self.after_edit();
        }
    }

    // ---- doors ----

    pub(super) fn select_door(&mut self, row: usize) {
        self.ed_mut(|e| {
            e.selected_door = Some(row);
            e.selected_object = None;
        });
        self.refresh_door_fields();
        self.h.map.get().invalidate();
    }

    pub(super) fn door_field(&mut self, field: DoorField, value: f64) {
        let value = to_u16(value);
        let changed = self
            .ed_mut(|e| {
                let i = e.selected_door?;
                let d = *e.room().doors().get(i)?;
                let new = match field {
                    DoorField::DestRoom => {
                        // Stored relative to the level's first room.
                        let level = e.level()?;
                        Door {
                            dest_room: Door::relative_to(level, value.into())?,
                            ..d
                        }
                    }
                    DoorField::Raw => Door {
                        dest_room: value,
                        ..d
                    },
                    DoorField::X => Door { x: value, ..d },
                    DoorField::Y => Door { y: value, ..d },
                    DoorField::DestX => Door { dest_x: value, ..d },
                    DoorField::DestY => Door { dest_y: value, ..d },
                };
                Some(e.set_door(i, new))
            })
            .flatten()
            .unwrap_or(false);
        if changed {
            self.after_edit();
            return;
        }
        self.refresh_door_fields();
        if field == DoorField::DestRoom {
            self.explain_refused_room(value.into());
        }
    }

    /// Says why a destination room the door cannot store was refused.
    fn explain_refused_room(&mut self, room: usize) {
        let reason = self
            .ed(|e| {
                let level = e.level()?;
                Door::relative_to(level, room).is_none().then(|| {
                    format!(
                        "Room {room} cannot be stored: doors count from {}'s first room ({})",
                        level.name, level.rooms.start
                    )
                })
            })
            .flatten();
        if let Some(reason) = reason {
            self.say(&reason);
        }
    }

    pub(super) fn goto_door_destination(&mut self) {
        let target = self
            .ed(|e| {
                let door = *e.room().doors().get(e.selected_door?)?;
                Some((door, e.destination(&door), e.room_count()))
            })
            .flatten();
        let Some((door, destination, count)) = target else {
            self.say("Select a door first");
            return;
        };
        let dest = match destination {
            Some(Destination::Room(room)) => room,
            Some(Destination::LevelExit) => {
                self.say("This is the stage's goal door: it leaves the level");
                return;
            }
            Some(Destination::Special(v)) => {
                self.say(&format!(
                    "Destination {v:04X} is a special value, not a room"
                ));
                return;
            }
            None => door.dest_room.into(),
        };
        if dest >= count {
            self.say(&format!(
                "Door leads to room {dest}, which does not exist ({count} rooms)"
            ));
            return;
        }
        if self.load_room(dest) {
            let arrival = (i32::from(door.dest_x), i32::from(door.dest_y));
            self.h.map.get().update_view(|v| v.center_on(arrival));
            self.say(&format!(
                "Room {dest}: arrival point ({}, {})",
                door.dest_x, door.dest_y
            ));
        }
    }

    /// The room position at the middle of the visible part of the map.
    fn view_centre(&self) -> (i32, i32) {
        let map = self.h.map.get();
        let v = map.view();
        let view = v.scroller.view();
        v.to_room(view.width() / 2, view.height() / 2)
    }

    pub(super) fn add_door(&mut self, warp: bool) {
        let pos = self.view_centre();
        let added = self
            .ed_mut(|e| {
                if warp {
                    e.add_warp(pos.0, pos.1)
                } else {
                    e.add_door(pos.0, pos.1)
                }
            })
            .flatten();
        match added {
            Some(index) => {
                self.after_edit();
                self.say(&if warp {
                    format!("Added warp star and door {index}: set where it leads")
                } else {
                    format!("Added door {index} with its door star: set where it leads")
                });
            }
            None => self.say("Could not add a door to this room"),
        }
    }

    pub(super) fn delete_door(&mut self) {
        let removed = self
            .ed_mut(|e| e.selected_door.is_some_and(|i| e.remove_door(i)))
            .unwrap_or(false);
        if removed {
            self.after_edit();
            self.say("Door deleted");
        } else {
            self.say("Select a door first");
        }
    }

    // ---- treasures ----

    /// Shows the chest of the picked treasure: its room, centred, selected.
    pub(super) fn select_treasure(&mut self, row: usize) {
        let Some(&number) = self.treasure_rows.get(row) else {
            return;
        };
        self.treasure = Some(number);
        let spot = self
            .ed(|e| e.chests().into_iter().find(|c| c.treasure == number))
            .flatten();
        let Some(spot) = spot else {
            self.refresh_treasures();
            return;
        };
        if !self.load_room(spot.room) {
            return;
        }
        self.ed_mut(|e| {
            e.selected_object = Some(spot.object);
            e.selected_door = None;
        });
        let chest = (i32::from(spot.x), i32::from(spot.y));
        self.h.map.get().update_view(|v| v.center_on(chest));
        self.refresh_objects();
        self.refresh_status();
        self.h.map.get().invalidate();
    }

    /// Arms the Object tool with a chest of the picked treasure.
    pub(super) fn place_treasure(&mut self) {
        let Some(number) = self.treasure else {
            self.say("Pick a treasure in the list first");
            return;
        };
        self.ed_mut(|e| e.new_object = (TREASURE_CHEST, number));
        self.set_tool(Tool::AddObject);
        self.refresh_objects();
        let in_cave = self
            .ed(|e| e.cave_rooms().contains(&e.current_index()))
            .unwrap_or(false);
        self.say(&if in_cave {
            format!("Click on the map to place a chest of treasure {number}")
        } else {
            format!(
                "Click on the map to place a chest of treasure {number} (chests are only counted in the cave's rooms)"
            )
        });
    }

    // ---- room ----

    pub(super) fn set_music(&mut self, value: f64) {
        if self.ed_mut(|e| e.set_music(to_u8(value))).unwrap_or(false) {
            self.after_edit();
        }
    }

    pub(super) fn resize_room(&mut self) {
        let (w, h) = self.size;
        if self.ed_mut(|e| e.resize(w, h)).unwrap_or(false) {
            self.after_edit();
            self.refresh_tree();
            self.say(&format!("Room resized to {w}x{h}"));
        }
    }
}
