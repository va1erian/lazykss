//! The editing model: a ROM, its loaded rooms and everything the UI changes.
//!
//! Nothing here knows about windows. The UI turns mouse and keyboard input
//! into calls on [`Editor`]; every call that changes a room is undoable and
//! marks the room as modified.
//!
//! The methods of [`Editor`] are spread over submodules by what they edit:
//! `rooms` (which rooms are loaded and current), `undo` (gestures, undo and
//! redo), `tiles`, `objects`, `doors`, `room_settings` (music and size) and
//! `saving`.

mod doors;
mod objects;
mod room_settings;
mod rooms;
mod saving;
mod tiles;
mod undo;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use kss_core::room::{GREAT_CAVE_OFFENSIVE, NAMED_LEVELS};
use kss_core::{Level, ObjectSprites, Rom, Room, Tables, Tileset};

use crate::history::{Entry, MapSnapshot};
use rooms::Rooms;

pub use saving::{SaveSummary, default_edited_path};
pub use tiles::line_cells;

/// The active editing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Paint,
    Rect,
    Fill,
    AddObject,
}

impl Tool {
    pub const ALL: [Tool; 5] = [
        Tool::Select,
        Tool::Paint,
        Tool::Rect,
        Tool::Fill,
        Tool::AddObject,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Paint => "Paint",
            Tool::Rect => "Rect",
            Tool::Fill => "Fill",
            Tool::AddObject => "Object",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => "Select / move objects (V)",
            Tool::Paint => "Paint tiles; right-click or Alt+click picks (B)",
            Tool::Rect => "Fill a rectangle of cells (R)",
            Tool::Fill => "Flood fill (G)",
            Tool::AddObject => "Place a new object (O)",
        }
    }
}

/// A room of the ROM with its tileset, edit history and modified flag.
pub struct LoadedRoom {
    pub room: Room,
    pub tileset: Tileset,
    /// The map as loaded or last written to a file: what the room's list of
    /// breakable cells was built for, and the records it had.
    saved: MapSnapshot,
    /// Bumped whenever the room image changes, to invalidate renders.
    pub revision: u64,
    pub undo: Vec<Entry>,
    pub redo: Vec<Entry>,
    pub dirty: bool,
}

/// A treasure chest placed in a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChestSpot {
    /// The treasure number (the chest's variant).
    pub treasure: u8,
    pub room: usize,
    /// Index in the room's object list.
    pub object: usize,
    pub x: u16,
    pub y: u16,
}

pub struct Editor {
    rom: Rom,
    tables: Tables,
    /// Distinguishes this editor from one opened earlier in the process,
    /// for caches keyed by room.
    session: u64,
    /// Object sprites decoded from the ROM, for the map.
    sprites: ObjectSprites,
    source: PathBuf,
    save_path: Option<PathBuf>,
    /// The loaded rooms. One of them is always current, so the accessors
    /// of the current room need no "not loaded" case.
    rooms: Rooms,
    pub tool: Tool,
    /// The metatile the paint and fill tools place.
    pub tile: u16,
    /// Id and variant of the next object the Object tool places.
    pub new_object: (u8, u8),
    pub selected_object: Option<usize>,
    pub selected_door: Option<usize>,
    group: Option<Entry>,
}

static NEXT_SESSION: AtomicU64 = AtomicU64::new(0);

impl Editor {
    /// Loads the ROM at `path` and opens room 1 (or room 0).
    pub fn open(path: &Path) -> Result<Editor, String> {
        let rom = Rom::load(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Editor::from_rom(rom, path.to_path_buf())
    }

    /// An editor on `rom` with room 1 (or room 0) current. Fails when that
    /// room cannot be read.
    pub fn from_rom(rom: Rom, source: PathBuf) -> Result<Editor, String> {
        let tables = Tables::load(&rom);
        let first = if tables.room_count() > 1 { 1 } else { 0 };
        let active = rooms::load_room(&rom, &tables, first)?;
        let sprites = ObjectSprites::load(&rom);
        let mut editor = Editor {
            rom,
            tables,
            session: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            sprites,
            source,
            save_path: None,
            rooms: Rooms::new(first, active),
            tool: Tool::Select,
            tile: 0,
            new_object: (0x01, 0),
            selected_object: None,
            selected_door: None,
            group: None,
        };
        // The named levels are listed with their sizes, and the treasure
        // list covers every room of the cave: load them now. A room that
        // cannot be read is left out; selecting it reports why.
        for index in editor.named_rooms() {
            if editor.rooms.get(index).is_none()
                && let Ok(room) = rooms::load_room(&editor.rom, &editor.tables, index)
            {
                editor.rooms.insert(index, room);
            }
        }
        Ok(editor)
    }

    // ---- ROM, tables, paths ----

    pub fn tables(&self) -> &Tables {
        &self.tables
    }

    pub fn room_count(&self) -> usize {
        self.tables.room_count()
    }

    /// The levels listed by name, in order.
    pub fn named_levels(&self) -> impl Iterator<Item = &Level> + '_ {
        NAMED_LEVELS
            .iter()
            .filter_map(|&i| self.tables.levels.get(i))
    }

    /// Every room of the named levels, in level order.
    pub fn named_rooms(&self) -> Vec<usize> {
        let count = self.tables.room_count();
        self.named_levels()
            .flat_map(|l| l.rooms.start.min(count)..l.rooms.end.min(count))
            .collect()
    }

    /// The rooms of The Great Cave Offensive.
    pub fn cave_rooms(&self) -> std::ops::Range<usize> {
        let count = self.tables.room_count();
        self.tables
            .levels
            .get(GREAT_CAVE_OFFENSIVE)
            .map_or(0..0, |l| l.rooms.start.min(count)..l.rooms.end.min(count))
    }

    /// The level of the current room.
    pub fn level(&self) -> Option<&Level> {
        self.tables.level_of(self.rooms.current())
    }

    /// The file the ROM was loaded from. Never written unless the user picks
    /// it in Save As.
    pub fn source_path(&self) -> &Path {
        &self.source
    }

    /// A number no other [`Editor`] of this process has: a view that caches
    /// what it drew for a room must also key it by this, or it shows the
    /// previous ROM's room after another ROM is opened.
    pub fn session(&self) -> u64 {
        self.session
    }

    pub fn sprites(&self) -> &ObjectSprites {
        &self.sprites
    }
}
