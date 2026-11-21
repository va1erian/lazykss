//! The application: messages, the widgets it updates and the reaction to each
//! message. The reactions are split by where the message comes from: the
//! file and edit commands (`commands.rs`), the map and its toolbars
//! (`map_input.rs`) and the side panels (`panels.rs`). Refreshing widgets
//! from the editor lives in `../sync.rs`.

mod commands;
mod map_input;
mod panels;

use std::path::PathBuf;

use xui::prelude::*;
use xui::widget::FileDialog;
use xui::xui_markdown::MarkdownView;

use super::map::{MapEvent, MapView};
use super::picker::TilePicker;
use super::tree::{self, GROUPS, Node};
use super::{Editor, SharedEditor};
use crate::editor::Tool;

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
    /// Shows the user guide (the Help tab).
    ShowGuide,
    /// Opens the settings window.
    ShowSettings,
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
    /// Painting, or emptying buffer cells when `erase`.
    Paint {
        last: (i32, i32),
        erase: bool,
    },
    Rect {
        start: (i32, i32),
    },
    /// Choosing the cells the stamp tool copies.
    Copy {
        start: (i32, i32),
    },
    /// Pasting the stamp: first at `origin`, then wherever a whole stamp
    /// fits from there as the pointer moves, so a drag repeats the motif.
    Stamp {
        origin: (i32, i32),
        last: (i32, i32),
    },
    Move {
        index: usize,
        dx: i32,
        dy: i32,
    },
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
    pub guide: Handle<MarkdownView<Msg>>,
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
            expanded: tree::initial_expansion(),
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

    /// Loads `room` and refreshes the window for it, or says why it failed.
    fn load_room(&mut self, room: usize) -> bool {
        match self.ed_mut(|e| e.select_room(room)) {
            Some(Ok(())) => {
                self.room_changed();
                true
            }
            Some(Err(e)) => {
                self.say(&format!("Could not load room {room}: {e}"));
                false
            }
            None => false,
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
            Msg::SaveAs => self.request_save_as(),
            Msg::SaveAsChosen(path) => self.save(Some(path)),
            Msg::Quit => self.quit(ui),
            Msg::Undo => self.undo_redo(false),
            Msg::Redo => self.undo_redo(true),
            Msg::SetTool(tool) => self.set_tool(tool),
            Msg::Layer(layer) => self.toggle_layer(layer),
            Msg::ZoomIn => self.zoom_by(1),
            Msg::ZoomOut => self.zoom_by(-1),
            Msg::ShowGuide => self.show_guide(),
            Msg::ShowSettings => self.show_settings(ui),
            Msg::BrowserSelect(id) => self.browser_select(id),
            Msg::BrowserToggle(id, open) => self.browser_toggle(id, open),
            Msg::Map(event) => self.map_event(event),
            Msg::PickTile(id) => {
                self.ed_mut(|e| e.tile = id);
                self.refresh_tile();
            }
            Msg::ObjectSelect(row) => self.select_object(row),
            Msg::ObjectField(field, value) => self.object_field(field, value),
            Msg::NewObjectField(field, value) => self.new_object_field(field, value),
            Msg::ObjectAdd => self.add_object_in_view(),
            Msg::ObjectDelete => self.delete_object(),
            Msg::DoorSelect(row) => self.select_door(row),
            Msg::DoorField(field, value) => self.door_field(field, value),
            Msg::DoorGoto => self.goto_door_destination(),
            Msg::DoorAdd => self.add_door(false),
            Msg::WarpAdd => self.add_door(true),
            Msg::DoorDelete => self.delete_door(),
            Msg::TreasureSelect(row) => self.select_treasure(row),
            Msg::TreasurePlace => self.place_treasure(),
            Msg::Music(value) => self.set_music(value),
            Msg::RoomWidth(value) => self.size.0 = value.round().max(1.0) as usize,
            Msg::RoomHeight(value) => self.size.1 = value.round().max(1.0) as usize,
            Msg::ResizeRoom => self.resize_room(),
        }
    }
}
