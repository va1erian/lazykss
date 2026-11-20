//! Pushing the editor's state into the widgets.

use kss_core::Destination;
use kss_core::names::song_name;
use kss_core::treasure;
use xui::prelude::*;

use super::app::EditorApp;
use super::tree::{self, Node};
use crate::editor::Tool;

impl EditorApp {
    /// A different room (or a different ROM) is now current.
    pub(super) fn room_changed(&mut self) {
        self.gesture = super::app::Gesture::Idle;
        self.cursor = None;
        if let Some((w, h)) = self.ed(|e| (e.room().width, e.room().height)) {
            self.h.map.get().update_view(|v| {
                v.set_room(w, h);
                v.scroller.offset = (0, 0);
                v.hover = None;
                v.rect_drag = None;
            });
        }
        self.refresh_all();
    }

    pub(super) fn refresh_all(&mut self) {
        self.refresh_tree();
        self.refresh_toolbar();
        self.refresh_history_buttons();
        self.refresh_tile();
        self.refresh_objects();
        self.refresh_doors();
        self.refresh_room_tab();
        self.refresh_treasures();
        self.refresh_status();
        self.h.map.get().invalidate();
    }

    // ---- browser ----

    pub(super) fn refresh_tree(&mut self) {
        let Some((rows, nodes, current, dirty)) = self.ed(|e| {
            let (rows, nodes) = tree::rows(e, &self.expanded);
            (rows, nodes, e.current_index(), e.dirty_rooms())
        }) else {
            return;
        };
        self.dirty_seen = dirty;
        let tree = self.h.tree.get();
        if rows != self.tree_rows {
            // Labels change under the user (a `*`, a size): keep the scroll.
            tree.refresh_rows(&rows);
            self.tree_rows = rows;
        }
        self.tree_nodes = nodes;
        tree.select(
            self.tree_nodes
                .iter()
                .position(|n| *n == Node::Room(current)),
        );
    }

    pub(super) fn refresh_tree_if_dirty_changed(&mut self) {
        if self
            .ed(|e| e.dirty_rooms())
            .is_some_and(|d| d != self.dirty_seen)
        {
            self.refresh_tree();
        }
    }

    // ---- toolbar ----

    pub(super) fn refresh_toolbar(&self) {
        let Some(tool) = self.ed(|e| e.tool) else {
            return;
        };
        // One tool at a time: the strip's items are plain toggles.
        for (index, t) in Tool::ALL.iter().enumerate() {
            self.h.tools.get().set_checked(index, *t == tool);
        }
    }

    pub(super) fn refresh_history_buttons(&self) {
        let (undo, redo) = self
            .ed(|e| (e.can_undo(), e.can_redo()))
            .unwrap_or((false, false));
        self.h.undo.get().set_enabled(undo);
        self.h.redo.get().set_enabled(redo);
    }

    // ---- tiles ----

    pub(super) fn refresh_tile(&self) {
        let Some((tile, attr)) = self.ed(|e| (e.tile, e.tileset().attribute(e.tile))) else {
            return;
        };
        self.h
            .tile_info
            .get()
            .set_text(&format!("Tile 0x{tile:03X}   attribute 0x{attr:02X}"));
        let picker = self.h.picker.get();
        picker.reveal(tile);
        picker.invalidate();
    }

    // ---- objects ----

    pub(super) fn refresh_objects(&self) {
        let Some((rows, selected, new)) = self.ed(|e| {
            let rows: Vec<Vec<String>> = e
                .room()
                .objects
                .iter()
                .map(|o| {
                    vec![
                        format!("{:02X}", o.id),
                        e.object_label(o),
                        format!("{:02X}", o.variant),
                        o.x.to_string(),
                        o.y.to_string(),
                    ]
                })
                .collect();
            (rows, e.selected_object, e.new_object)
        }) else {
            return;
        };
        let list = self.h.objects.get();
        list.refresh_model(rows);
        list.select(selected);
        // Scrolling a list that is on a hidden tab page would leave it
        // scrolled past its rows once shown, so only do it while visible.
        if let Some(row) = selected.filter(|_| self.h.tabs.get().selected() == 1) {
            list.ensure_visible(row);
        }
        self.h.new_object_fields[0].get().set_value(new.0 as f64);
        self.h.new_object_fields[1].get().set_value(new.1 as f64);
        self.refresh_object_fields();
    }

    pub(super) fn refresh_object_fields(&self) {
        let object = self
            .ed(|e| {
                e.selected_object
                    .and_then(|i| e.room().objects.get(i).copied())
            })
            .flatten();
        let values = object.map_or([0.0; 4], |o| {
            [o.id as f64, o.variant as f64, o.x as f64, o.y as f64]
        });
        for (field, value) in self.h.object_fields.iter().zip(values) {
            let field = field.get();
            field.set_value(value);
            field.set_enabled(object.is_some());
        }
    }

    /// After an object was added, shows it selected in the list and fields.
    pub(super) fn select_object_row(&self) {
        self.refresh_objects();
    }

    // ---- doors ----

    pub(super) fn refresh_doors(&self) {
        let Some((rows, selected)) = self.ed(|e| {
            let rows: Vec<Vec<String>> = e
                .room()
                .doors()
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    let warps = e.warps_of(i);
                    vec![
                        i.to_string(),
                        e.destination_label(d),
                        if d.has_trigger() {
                            format!("{}, {}", d.x, d.y)
                        } else if !warps.is_empty() {
                            "warp star".to_string()
                        } else {
                            "none".to_string()
                        },
                        format!("{}, {}", d.dest_x, d.dest_y),
                    ]
                })
                .collect();
            (rows, e.selected_door)
        }) else {
            return;
        };
        let list = self.h.doors.get();
        list.refresh_model(rows);
        list.select(selected);
        self.refresh_door_fields();
    }

    pub(super) fn refresh_door_fields(&self) {
        let info = self
            .ed(|e| {
                let door = e
                    .selected_door
                    .and_then(|i| e.room().doors().get(i).copied().map(|d| (i, d)));
                let level = e.level().map(|l| (l.name.clone(), l.rooms.clone()));
                door.map(|(i, d)| (d, e.destination(&d), e.warps_of(i).len(), level))
            })
            .flatten();
        let count = self.ed(|e| e.room_count()).unwrap_or(0);
        let door = info.as_ref().map(|i| i.0);
        let room = match info.as_ref().and_then(|i| i.1) {
            Some(Destination::Room(r)) => Some(r),
            _ => None,
        };
        let values = door.map_or([0.0; 6], |d| {
            [
                room.unwrap_or(0) as f64,
                d.dest_room as f64,
                d.x as f64,
                d.y as f64,
                d.dest_x as f64,
                d.dest_y as f64,
            ]
        });
        for (i, (field, value)) in self.h.door_fields.iter().zip(values).enumerate() {
            let field = field.get();
            field.set_value(value);
            // The room field only means something for a door to a room.
            field.set_enabled(door.is_some() && (i != 0 || room.is_some()));
        }
        let note = match info {
            None => "Select a door in the list or on the map.".to_string(),
            Some((d, destination, warps, level)) => {
                // One line fits the panel: the most important thing to say.
                match (destination, &level) {
                    (Some(Destination::Room(r)), _) if r >= count => {
                        format!("Warning: room {r} does not exist.")
                    }
                    (Some(Destination::Room(r)), Some((name, rooms))) if !rooms.contains(&r) => {
                        format!("Warning: room {r} is outside {name}.")
                    }
                    (Some(Destination::LevelExit), _) => {
                        "The stage's goal door (stored 7FFF).".to_string()
                    }
                    (Some(Destination::Special(v)), _) => {
                        format!("Stored {v:04X}: not a room (meaning unknown).")
                    }
                    _ if !d.has_trigger() => match warps {
                        0 => "No trigger, and no warp star uses it.".to_string(),
                        1 => "No trigger: a warp star in this room uses it.".to_string(),
                        n => format!("No trigger: {n} warp stars use it."),
                    },
                    (_, Some((_, rooms))) => {
                        format!("Stored counts from room {} (level start).", rooms.start)
                    }
                    _ => String::new(),
                }
            }
        };
        self.h.door_note.get().set_text(&note);
    }

    // ---- treasures ----

    pub(super) fn refresh_treasures(&mut self) {
        let Some((chests, current)) = self.ed(|e| (e.chests(), e.current_index())) else {
            return;
        };
        let mut numbers: Vec<u8> = (0..treasure::COUNT as u8).collect();
        for c in &chests {
            if !numbers.contains(&c.treasure) {
                numbers.push(c.treasure);
            }
        }
        let rows: Vec<Vec<String>> = numbers
            .iter()
            .map(|&n| {
                let here: Vec<_> = chests.iter().filter(|c| c.treasure == n).collect();
                let (name, value) = match treasure::treasure(n) {
                    Some(t) => (t.name.to_string(), format!("{}G", t.value)),
                    None => ("(no such treasure)".to_string(), String::new()),
                };
                let room = match here.as_slice() {
                    [] => "no chest".to_string(),
                    [one] if one.room == current => format!("{} (here)", one.room),
                    [one] => one.room.to_string(),
                    many => format!("{} chests!", many.len()),
                };
                vec![n.to_string(), name, value, room]
            })
            .collect();
        let list = self.h.treasures.get();
        list.refresh_model(rows);
        list.select(
            self.treasure
                .and_then(|t| numbers.iter().position(|&n| n == t)),
        );
        self.treasure_rows = numbers;
        let placed = (0..treasure::COUNT as u8)
            .filter(|n| chests.iter().any(|c| c.treasure == *n))
            .count();
        let note = match self.treasure {
            Some(n) => {
                let here: Vec<_> = chests.iter().filter(|c| c.treasure == n).collect();
                match here.as_slice() {
                    [] => format!("Treasure {n} has no chest in the cave."),
                    [c] => format!("Treasure {n}: room {}, at ({}, {}).", c.room, c.x, c.y),
                    many => {
                        let rooms: Vec<String> = many.iter().map(|c| c.room.to_string()).collect();
                        format!(
                            "Treasure {n} is in {} chests: rooms {}.",
                            many.len(),
                            rooms.join(", ")
                        )
                    }
                }
            }
            None => format!(
                "{placed} of {} treasures have a chest in the cave.",
                treasure::COUNT
            ),
        };
        self.h.treasure_note.get().set_text(&note);
    }

    // ---- room tab ----

    pub(super) fn refresh_room_tab(&mut self) {
        let Some((music, width, height, head)) = self.ed(|e| {
            let r = e.room();
            (r.music(), r.width, r.height, r.head().to_vec())
        }) else {
            return;
        };
        self.h.music.get().set_value(music as f64);
        self.h
            .music_name
            .get()
            .set_text(song_name(music).unwrap_or("(unnamed song)"));
        self.size = (width, height);
        self.h.size_fields[0].get().set_value(width as f64);
        self.h.size_fields[1].get().set_value(height as f64);
        let rows: Vec<Vec<String>> = head
            .chunks(16)
            .enumerate()
            .map(|(i, chunk)| {
                let bytes: Vec<String> = chunk.iter().map(|b| format!("{b:02X}")).collect();
                vec![format!("{:04X}  {}", i * 16, bytes.join(" "))]
            })
            .collect();
        self.h.head.get().refresh_model(rows);
    }

    // ---- status ----

    pub(super) fn refresh_status(&self) {
        let info = self.h.info.get();
        let view = self.h.map.get().view();
        let Some((room, level, dims, tile, hovered, tool, objects)) = self.ed(|e| {
            let level = e
                .tables()
                .levels
                .iter()
                .find(|l| l.rooms.contains(&e.current_index()))
                .map(|l| l.name.clone())
                .unwrap_or_default();
            let hovered = self
                .cursor
                .and_then(|(cell, _)| e.tile_at(cell.0, cell.1).map(|t| (cell, t)));
            (
                e.current_index(),
                level,
                (e.room().width, e.room().height),
                e.tile,
                hovered,
                e.tool,
                e.room().objects.len(),
            )
        }) else {
            info.set_text(0, "No ROM loaded");
            info.set_text(1, "");
            info.set_text(2, "");
            return;
        };
        let level = if level.is_empty() {
            String::new()
        } else {
            format!(" ({level})")
        };
        info.set_text(
            0,
            &format!(
                "Room {room}{level}  {}x{}  {objects} objects",
                dims.0, dims.1
            ),
        );
        let cursor = match (self.cursor, hovered) {
            (Some((_, pos)), Some((cell, t))) => {
                format!(
                    "cell ({}, {})  px ({}, {})  tile 0x{t:03X}",
                    cell.0, cell.1, pos.0, pos.1
                )
            }
            (Some((_, pos)), None) => format!("outside room  px ({}, {})", pos.0, pos.1),
            _ => String::from("cell -"),
        };
        info.set_text(1, &cursor);
        let tool = if tool == Tool::Select {
            String::new()
        } else {
            format!("  {}", tool.label())
        };
        info.set_text(2, &format!("Zoom {}x  tile 0x{tile:03X}{tool}", view.zoom));
        self.h.zoom_label.get().set_text(&format!("{}x", view.zoom));
    }
}
