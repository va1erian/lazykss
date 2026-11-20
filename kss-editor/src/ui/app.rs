//! The application: messages, the widgets it updates and the reaction to each
//! message. Refreshing widgets from the editor lives in `sync.rs`.

use std::path::PathBuf;

use kss_core::names::TREASURE_CHEST;
use kss_core::{Destination, Door, Object};
use xui::prelude::*;
use xui::widget::FileDialog;

use super::keys::{self, MapKey};
use super::map::{MapEvent, MapView};
use super::picker::TilePicker;
use super::tree::{self, GROUPS, Node};
use super::{Editor, SharedEditor};
use crate::editor::{Tool, line_cells};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjField {
    Id,
    Variant,
    X,
    Y,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorField {
    /// The destination as a room of the pointer table.
    DestRoom,
    /// The stored value: level-relative, or a special code.
    Raw,
    X,
    Y,
    DestX,
    DestY,
}

#[derive(Clone)]
pub enum Msg {
    // file
    OpenRom,
    RomChosen(PathBuf),
    Save,
    SaveAs,
    SaveAsChosen(PathBuf),
    Quit,
    // edit and view
    Undo,
    Redo,
    SetTool(Tool),
    Layer(Layer),
    ZoomIn,
    ZoomOut,
    // browser, map, picker
    BrowserSelect(usize),
    BrowserToggle(usize, bool),
    Map(MapEvent),
    PickTile(u16),
    // objects tab
    ObjectSelect(usize),
    ObjectField(ObjField, f64),
    NewObjectField(ObjField, f64),
    ObjectAdd,
    ObjectDelete,
    // doors tab
    DoorSelect(usize),
    DoorField(DoorField, f64),
    DoorGoto,
    DoorAdd,
    WarpAdd,
    DoorDelete,
    // treasures tab
    TreasureSelect(usize),
    TreasurePlace,
    // room tab
    Music(f64),
    RoomWidth(f64),
    RoomHeight(f64),
    ResizeRoom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Background,
    Foreground,
    Grid,
    Doors,
    Objects,
}

impl Layer {
    /// In toolbar order.
    pub const ALL: [Layer; 5] = [
        Layer::Background,
        Layer::Foreground,
        Layer::Grid,
        Layer::Doors,
        Layer::Objects,
    ];
}

/// What the pointer is doing on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Idle,
    Paint { last: (i32, i32) },
    Rect { start: (i32, i32) },
    Move { index: usize, dx: i32, dy: i32 },
}

/// Handles of every widget the app updates after the layout is mounted.
#[derive(Default)]
pub struct Handles {
    pub map: Handle<MapView<Msg>>,
    pub picker: Handle<TilePicker<Msg>>,
    pub tree: Handle<TreeView<Msg>>,
    pub tabs: Handle<Tabs<Msg>>,
    /// The tool strip: one checkable item per `Tool::ALL` entry.
    pub tools: Handle<Toolbar<Msg>>,
    /// The layer strip: one checkable item per `Layer::ALL` entry.
    pub layers: Handle<Toolbar<Msg>>,
    pub undo: Handle<Button<Msg>>,
    pub redo: Handle<Button<Msg>>,
    pub zoom_label: Handle<Label<Msg>>,
    pub info: Handle<StatusBar<Msg>>,
    pub message: Handle<StatusBar<Msg>>,
    pub tile_info: Handle<Label<Msg>>,
    pub objects: Handle<ListView<Msg>>,
    pub object_fields: [Handle<NumberField<Msg>>; 4],
    pub new_object_fields: [Handle<NumberField<Msg>>; 2],
    pub doors: Handle<ListView<Msg>>,
    pub door_fields: [Handle<NumberField<Msg>>; 6],
    pub door_note: Handle<Label<Msg>>,
    pub treasures: Handle<ListView<Msg>>,
    pub treasure_note: Handle<Label<Msg>>,
    pub music: Handle<NumberField<Msg>>,
    pub music_name: Handle<Label<Msg>>,
    pub size_fields: [Handle<NumberField<Msg>>; 2],
    pub head: Handle<ListView<Msg>>,
}

pub struct EditorApp {
    pub(super) editor: SharedEditor,
    pub(super) h: Handles,
    pub(super) open_dialog: FileDialog<Msg>,
    pub(super) save_dialog: FileDialog<Msg>,
    pub(super) gesture: Gesture,
    pub(super) expanded: [bool; GROUPS],
    pub(super) tree_rows: Vec<xui::widget::TreeRow>,
    pub(super) tree_nodes: Vec<Node>,
    pub(super) dirty_seen: Vec<usize>,
    /// Pointer position over the map, for the status bar.
    pub(super) cursor: Option<((i32, i32), (i32, i32))>,
    pub(super) size: (usize, usize),
    pub(super) discard_armed: bool,
    /// The treasure number of each row of the treasure list.
    pub(super) treasure_rows: Vec<u8>,
    /// The treasure picked in the list.
    pub(super) treasure: Option<u8>,
}

impl EditorApp {
    pub(super) fn new(
        editor: SharedEditor,
        h: Handles,
        open_dialog: FileDialog<Msg>,
        save_dialog: FileDialog<Msg>,
    ) -> Self {
        Self {
            editor,
            h,
            open_dialog,
            save_dialog,
            gesture: Gesture::Idle,
            expanded: Self::new_expansion(),
            tree_rows: Vec::new(),
            tree_nodes: Vec::new(),
            dirty_seen: Vec::new(),
            cursor: None,
            size: (1, 1),
            discard_armed: false,
            treasure_rows: Vec::new(),
            treasure: None,
        }
    }

    pub(super) fn ed<R>(&self, f: impl FnOnce(&Editor) -> R) -> Option<R> {
        self.editor.borrow().as_ref().map(f)
    }

    pub(super) fn ed_mut<R>(&self, f: impl FnOnce(&mut Editor) -> R) -> Option<R> {
        self.editor.borrow_mut().as_mut().map(f)
    }

    pub(super) fn say(&self, text: &str) {
        self.h.message.get().set_text(0, text);
    }

    // ---- file ----

    fn open_rom(&mut self, path: PathBuf) {
        match Editor::open(&path) {
            Ok(editor) => {
                *self.editor.borrow_mut() = Some(editor);
                self.expanded = Self::new_expansion();
                self.dirty_seen.clear();
                self.tree_rows.clear();
                self.room_changed();
                self.say(&format!("Opened {}", path.display()));
            }
            Err(e) => self.say(&format!("Could not open ROM: {e}")),
        }
    }

    fn new_expansion() -> [bool; GROUPS] {
        tree::initial_expansion()
    }

    fn save(&mut self, as_path: Option<PathBuf>) {
        let result = self.ed_mut(|e| match &as_path {
            Some(path) => e.save_as(path),
            None => e.save(),
        });
        match result {
            Some(Ok(summary)) => {
                self.say(&summary.describe());
                self.refresh_tree();
            }
            Some(Err(e)) => self.say(&format!("Save failed: {e}")),
            None => self.say("Nothing to save: no ROM is open"),
        }
    }

    /// Open ROM: asks for the file, but first warns once about unsaved edits.
    fn request_open(&mut self) {
        if self.ed(|e| e.any_dirty()).unwrap_or(false) && !self.discard_armed {
            self.discard_armed = true;
            self.say("Unsaved changes. Save first, or choose Open ROM again to discard them.");
            return;
        }
        self.discard_armed = false;
        if let Some(dir) = self
            .ed(|e| e.source_path().parent().map(|p| p.to_path_buf()))
            .flatten()
        {
            self.open_dialog.set_initial_dir(dir);
        }
        self.open_dialog.open();
    }

    // ---- editing helpers ----

    /// Refreshes whatever an edit can change, except the heavy lists.
    pub(super) fn after_stroke(&mut self) {
        self.h.map.get().invalidate();
        self.refresh_tree_if_dirty_changed();
        self.refresh_history_buttons();
    }

    /// Refreshes everything an edit can change.
    pub(super) fn after_edit(&mut self) {
        self.after_stroke();
        self.refresh_objects();
        self.refresh_doors();
        self.refresh_room_tab();
        self.refresh_treasures();
        self.refresh_status();
    }

    fn set_tool(&mut self, tool: Tool) {
        self.ed_mut(|e| {
            e.end_group();
            e.tool = tool;
        });
        self.gesture = Gesture::Idle;
        self.h.map.get().update_view(|v| v.rect_drag = None);
        self.refresh_toolbar();
        self.say(tool.hint());
    }

    fn zoom_by(&mut self, delta: i32) {
        self.h
            .map
            .get()
            .update_view(|v| v.set_zoom_centered(v.zoom + delta));
        self.refresh_status();
    }

    // ---- map input ----

    fn map_event(&mut self, event: MapEvent) {
        match event {
            MapEvent::Hover { cell, pos } => {
                self.cursor = Some((cell, pos));
                self.refresh_status();
            }
            MapEvent::Leave => {
                self.cursor = None;
                self.refresh_status();
            }
            MapEvent::ViewChanged => self.refresh_status(),
            MapEvent::Press {
                button,
                cell,
                pos,
                mods,
            } => self.press(button, cell, pos, mods),
            MapEvent::Drag { cell, pos, .. } => {
                self.cursor = Some((cell, pos));
                self.drag(cell, pos);
            }
            MapEvent::Release { cell, pos, .. } => {
                self.cursor = Some((cell, pos));
                self.release(cell);
            }
            MapEvent::Key { key, mods } => {
                if let Some(action) = keys::map_key(key, mods) {
                    self.map_key(action);
                }
            }
        }
    }

    fn press(&mut self, button: MouseButton, cell: (i32, i32), pos: (i32, i32), mods: Modifiers) {
        // With no ROM the map shows an "Open ROM" button; a click on it opens the dialog.
        if self.ed(|_| ()).is_none() {
            if button == MouseButton::Left {
                self.request_open();
            }
            return;
        }
        let zoom = self.h.map.get().view().zoom;
        let tool = self.ed(|e| e.tool).unwrap_or(Tool::Select);
        let tile_tool = matches!(tool, Tool::Paint | Tool::Rect | Tool::Fill);
        // Right-click, or Alt+click with the paint tool, picks the tile.
        if (button == MouseButton::Right && tile_tool) || (mods.alt && tool == Tool::Paint) {
            if self
                .ed_mut(|e| e.pick_tile(cell.0, cell.1))
                .unwrap_or(false)
            {
                self.refresh_tile();
            }
            return;
        }
        if button != MouseButton::Left {
            return;
        }
        match tool {
            Tool::Select => {
                let radius = 8 + 4 / zoom;
                let hit = self.ed(|e| e.object_at(pos.0, pos.1, radius)).flatten();
                match hit {
                    Some(index) => {
                        let o = self.ed_mut(|e| {
                            e.selected_object = Some(index);
                            e.selected_door = None;
                            e.begin_group();
                            e.room().objects[index]
                        });
                        if let Some(o) = o {
                            self.gesture = Gesture::Move {
                                index,
                                dx: o.x as i32 - pos.0,
                                dy: o.y as i32 - pos.1,
                            };
                        }
                    }
                    None => {
                        self.ed_mut(|e| {
                            e.selected_object = None;
                            e.selected_door = e.door_at(pos.0, pos.1);
                        });
                    }
                }
                self.refresh_objects();
                self.refresh_doors();
                self.h.map.get().invalidate();
            }
            Tool::Paint => {
                self.ed_mut(|e| {
                    e.begin_group();
                    e.paint(cell.0, cell.1);
                });
                self.gesture = Gesture::Paint { last: cell };
                self.after_stroke();
            }
            Tool::Rect => {
                self.gesture = Gesture::Rect { start: cell };
                self.h
                    .map
                    .get()
                    .update_view(|v| v.rect_drag = Some((cell, cell)));
            }
            Tool::Fill => {
                let n = self.ed_mut(|e| e.flood_fill(cell.0, cell.1)).unwrap_or(0);
                self.after_stroke();
                if n > 0 {
                    self.say(&format!("Filled {n} cells"));
                }
            }
            Tool::AddObject => {
                self.ed_mut(|e| e.add_object(pos.0, pos.1));
                self.after_edit();
                self.select_object_row();
            }
        }
    }

    fn drag(&mut self, cell: (i32, i32), pos: (i32, i32)) {
        match self.gesture {
            Gesture::Paint { last } => {
                if cell != last {
                    self.ed_mut(|e| {
                        for (x, y) in line_cells(last, cell) {
                            e.paint(x, y);
                        }
                    });
                    self.gesture = Gesture::Paint { last: cell };
                    self.after_stroke();
                }
            }
            Gesture::Rect { start } => {
                self.h
                    .map
                    .get()
                    .update_view(|v| v.rect_drag = Some((start, cell)));
            }
            Gesture::Move { index, dx, dy } => {
                self.ed_mut(|e| e.move_object(index, pos.0 + dx, pos.1 + dy));
                self.after_stroke();
                self.refresh_object_fields();
            }
            Gesture::Idle => {}
        }
        self.refresh_status();
    }

    fn release(&mut self, cell: (i32, i32)) {
        match std::mem::replace(&mut self.gesture, Gesture::Idle) {
            Gesture::Paint { .. } => {
                self.ed_mut(|e| e.end_group());
                self.after_stroke();
            }
            Gesture::Rect { start } => {
                let n = self.ed_mut(|e| e.fill_rect(start, cell)).unwrap_or(0);
                self.h.map.get().update_view(|v| v.rect_drag = None);
                self.after_stroke();
                if n > 0 {
                    self.say(&format!("Filled {n} cells"));
                }
            }
            Gesture::Move { .. } => {
                self.ed_mut(|e| e.end_group());
                self.after_edit();
            }
            Gesture::Idle => {}
        }
    }

    fn map_key(&mut self, key: MapKey) {
        match key {
            MapKey::Tool(tool) => self.set_tool(tool),
            MapKey::Zoom(z) => {
                self.h.map.get().update_view(|v| v.set_zoom_centered(z));
                self.refresh_status();
            }
            MapKey::Escape => {
                self.gesture = Gesture::Idle;
                self.ed_mut(|e| {
                    e.end_group();
                    e.selected_object = None;
                    e.selected_door = None;
                });
                self.h.map.get().update_view(|v| v.rect_drag = None);
                self.refresh_objects();
            }
            MapKey::DeleteObject => {
                // The selected object, else the selected door.
                let removed = self
                    .ed_mut(|e| match (e.selected_object, e.selected_door) {
                        (Some(i), _) => e.remove_object(i),
                        (None, Some(i)) => e.remove_door(i),
                        (None, None) => false,
                    })
                    .unwrap_or(false);
                if removed {
                    self.after_edit();
                }
            }
            MapKey::Nudge(dx, dy) => {
                let moved = self
                    .ed_mut(|e| e.selected_object.is_some_and(|i| e.nudge_object(i, dx, dy)))
                    .unwrap_or(false);
                if moved {
                    self.after_stroke();
                    self.refresh_object_fields();
                    self.refresh_objects();
                }
            }
        }
    }

    // ---- panels ----

    fn object_field(&mut self, field: ObjField, value: f64) {
        let value = value.round().clamp(0.0, 65535.0) as u16;
        let changed = self
            .ed_mut(|e| {
                let i = e.selected_object?;
                let o = *e.room().objects.get(i)?;
                let new = match field {
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

    fn door_field(&mut self, field: DoorField, value: f64) {
        let value = value.round().clamp(0.0, 65535.0) as u16;
        let changed = self
            .ed_mut(|e| {
                let i = e.selected_door?;
                let d = *e.room().doors().get(i)?;
                let new = match field {
                    DoorField::DestRoom => {
                        // Stored relative to the level's first room.
                        let level = e.level()?;
                        Door {
                            dest_room: Door::relative_to(level, value as usize)?,
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
        } else {
            self.refresh_door_fields();
        }
    }

    fn goto_door_destination(&mut self) {
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
            None => door.dest_room as usize,
        };
        if dest >= count {
            self.say(&format!(
                "Door leads to room {dest}, which does not exist ({count} rooms)"
            ));
            return;
        }
        match self.ed_mut(|e| e.select_room(dest)) {
            Some(Ok(())) => {
                self.room_changed();
                let map = self.h.map.get();
                map.update_view(|v| v.center_on((door.dest_x as i32, door.dest_y as i32)));
                self.say(&format!(
                    "Room {dest}: arrival point ({}, {})",
                    door.dest_x, door.dest_y
                ));
            }
            Some(Err(e)) => self.say(&format!("Could not load room {dest}: {e}")),
            None => {}
        }
    }

    /// The room position at the middle of the visible part of the map.
    fn view_centre(&self) -> (i32, i32) {
        let map = self.h.map.get();
        let v = map.view();
        let view = v.scroller.view();
        v.to_room(view.width() / 2, view.height() / 2)
    }

    fn add_door(&mut self, warp: bool) {
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

    /// Shows the chest of the picked treasure: its room, centred, selected.
    fn select_treasure(&mut self, row: usize) {
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
        match self.ed_mut(|e| e.select_room(spot.room)) {
            Some(Ok(())) => {
                self.room_changed();
                self.ed_mut(|e| {
                    e.selected_object = Some(spot.object);
                    e.selected_door = None;
                });
                self.h
                    .map
                    .get()
                    .update_view(|v| v.center_on((spot.x as i32, spot.y as i32)));
                self.refresh_objects();
                self.refresh_status();
                self.h.map.get().invalidate();
            }
            Some(Err(e)) => self.say(&format!("Could not load room {}: {e}", spot.room)),
            None => {}
        }
    }

    /// Arms the Object tool with a chest of the picked treasure.
    fn place_treasure(&mut self) {
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

    fn resize_room(&mut self) {
        let (w, h) = self.size;
        if self.ed_mut(|e| e.resize(w, h)).unwrap_or(false) {
            self.after_edit();
            self.refresh_tree();
            self.say(&format!("Room resized to {w}x{h}"));
        }
    }
}

impl App for EditorApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        // The warning about unsaved changes is confirmed by asking again
        // straight away; anything in between withdraws the confirmation.
        if !matches!(msg, Msg::OpenRom | Msg::Quit) {
            self.discard_armed = false;
        }
        match msg {
            Msg::OpenRom => self.request_open(),
            Msg::RomChosen(path) => self.open_rom(path),
            Msg::Save => self.save(None),
            Msg::SaveAs => {
                if let Some((dir, name)) = self.ed(|e| {
                    let target = crate::editor::default_edited_path(e.source_path());
                    (
                        target.parent().map(|p| p.to_path_buf()),
                        target.file_name().map(|n| n.to_string_lossy().into_owned()),
                    )
                }) {
                    if let Some(dir) = dir {
                        self.save_dialog.set_initial_dir(dir);
                    }
                    if let Some(name) = name {
                        self.save_dialog.set_suggested_name(&name);
                    }
                    self.save_dialog.open();
                }
            }
            Msg::SaveAsChosen(path) => self.save(Some(path)),
            Msg::Quit => {
                if self.ed(|e| e.any_dirty()).unwrap_or(false) && !self.discard_armed {
                    self.discard_armed = true;
                    self.say("Unsaved changes. Save first, or choose Quit again to discard them.");
                } else {
                    ui.quit();
                }
            }
            Msg::Undo | Msg::Redo => {
                self.gesture = Gesture::Idle;
                let redo = matches!(msg, Msg::Redo);
                let done = self
                    .ed_mut(|e| if redo { e.redo() } else { e.undo() })
                    .unwrap_or(false);
                if done {
                    self.after_edit();
                    self.say(if redo { "Redo" } else { "Undo" });
                }
            }
            Msg::SetTool(tool) => self.set_tool(tool),
            Msg::Layer(layer) => {
                // The view owns the state; the strip is set to match, whether
                // the message came from a click on it or from elsewhere.
                let mut on = false;
                self.h.map.get().update_view(|v| {
                    let flag = match layer {
                        Layer::Background => &mut v.layers.background,
                        Layer::Foreground => &mut v.layers.foreground,
                        Layer::Grid => &mut v.grid,
                        Layer::Doors => &mut v.doors,
                        Layer::Objects => &mut v.objects,
                    };
                    *flag = !*flag;
                    on = *flag;
                });
                let index = Layer::ALL.iter().position(|l| *l == layer).unwrap_or(0);
                self.h.layers.get().set_checked(index, on);
            }
            Msg::ZoomIn => self.zoom_by(1),
            Msg::ZoomOut => self.zoom_by(-1),
            Msg::BrowserSelect(id) => {
                if let Some(Node::Room(room)) = self.tree_nodes.get(id).copied() {
                    match self.ed_mut(|e| e.select_room(room)) {
                        Some(Ok(())) => self.room_changed(),
                        Some(Err(e)) => self.say(&format!("Could not load room {room}: {e}")),
                        None => {}
                    }
                }
            }
            Msg::BrowserToggle(id, open) => {
                if let Some(group) = self.tree_nodes.get(id).copied().and_then(tree::group_index) {
                    self.expanded[group] = open;
                }
            }
            Msg::Map(event) => self.map_event(event),
            Msg::PickTile(id) => {
                self.ed_mut(|e| e.tile = id);
                self.refresh_tile();
            }
            Msg::ObjectSelect(row) => {
                self.ed_mut(|e| {
                    e.selected_object = Some(row);
                    e.selected_door = None;
                });
                self.refresh_object_fields();
                self.h.map.get().invalidate();
            }
            Msg::ObjectField(field, value) => self.object_field(field, value),
            Msg::NewObjectField(field, value) => {
                let value = value.round().clamp(0.0, 255.0) as u8;
                self.ed_mut(|e| match field {
                    ObjField::Variant => e.new_object.1 = value,
                    _ => e.new_object.0 = value,
                });
            }
            Msg::ObjectAdd => {
                // Place it in the middle of what is visible.
                let pos = self.view_centre();
                self.ed_mut(|e| e.add_object(pos.0, pos.1));
                self.after_edit();
                self.select_object_row();
            }
            Msg::ObjectDelete => {
                let removed = self
                    .ed_mut(|e| e.selected_object.is_some_and(|i| e.remove_object(i)))
                    .unwrap_or(false);
                if removed {
                    self.after_edit();
                }
            }
            Msg::DoorSelect(row) => {
                self.ed_mut(|e| {
                    e.selected_door = Some(row);
                    e.selected_object = None;
                });
                self.refresh_door_fields();
                self.h.map.get().invalidate();
            }
            Msg::DoorField(field, value) => self.door_field(field, value),
            Msg::DoorGoto => self.goto_door_destination(),
            Msg::DoorAdd => self.add_door(false),
            Msg::WarpAdd => self.add_door(true),
            Msg::DoorDelete => {
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
            Msg::TreasureSelect(row) => self.select_treasure(row),
            Msg::TreasurePlace => self.place_treasure(),
            Msg::Music(value) => {
                if self
                    .ed_mut(|e| e.set_music(value.round().clamp(0.0, 255.0) as u8))
                    .unwrap_or(false)
                {
                    self.after_edit();
                }
            }
            Msg::RoomWidth(value) => self.size.0 = value.round().max(1.0) as usize,
            Msg::RoomHeight(value) => self.size.1 = value.round().max(1.0) as usize,
            Msg::ResizeRoom => self.resize_room(),
        }
    }
}
