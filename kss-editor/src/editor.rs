//! The editing model: a ROM, its loaded rooms and everything the UI changes.
//!
//! Nothing here knows about windows. The UI turns mouse and keyboard input
//! into calls on [`Editor`]; every call that changes a room is undoable and
//! marks the room as modified.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kss_core::names::{DOOR_STAR, TREASURE_CHEST, WARP_STAR};
use kss_core::room::{Destination, GREAT_CAVE_OFFENSIVE, MAX_CELLS, MAX_DOORS, NAMED_LEVELS};
use kss_core::{CELL, Door, Level, Object, ObjectSprites, Rom, Room, SaveReport, Tables, Tileset};

use crate::history::{Change, Entry, MapSnapshot, TileChange};

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

/// What a save wrote.
#[derive(Debug, Clone)]
pub struct SaveSummary {
    pub path: PathBuf,
    /// The rooms written and what the save did to each.
    pub rooms: Vec<(usize, SaveReport)>,
}

impl SaveSummary {
    /// A one-line description for the status bar.
    pub fn describe(&self) -> String {
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if self.rooms.is_empty() {
            return format!("Saved {name} (no room changes)");
        }
        let rooms: Vec<String> = self
            .rooms
            .iter()
            .map(|(room, r)| {
                let moved = if r.moved { ", moved" } else { "" };
                format!(
                    "room {room}: {} bytes (was {}{moved})",
                    r.len, r.previous_len
                )
            })
            .collect();
        format!("Saved {name}: {}", rooms.join("; "))
    }
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
    rooms: BTreeMap<usize, LoadedRoom>,
    current: usize,
    pub tool: Tool,
    /// The metatile the paint and fill tools place.
    pub tile: u16,
    /// Id and variant of the next object the Object tool places.
    pub new_object: (u8, u8),
    pub selected_object: Option<usize>,
    pub selected_door: Option<usize>,
    group: Option<Entry>,
}

/// The cells on the line between two cells, both included, so a fast drag
/// paints without gaps.
pub fn line_cells(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
    let (sx, sy) = (
        if a.0 < b.0 { 1 } else { -1 },
        if a.1 < b.1 { 1 } else { -1 },
    );
    let (mut x, mut y, mut err) = (a.0, a.1, dx + dy);
    let mut cells = vec![(x, y)];
    while (x, y) != b {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        cells.push((x, y));
    }
    cells
}

/// `<name> (edited).sfc` next to `source`, or `(edited 2)`, `(edited 3)`...
/// when that exists: a plain Save in a new session never replaces a file
/// written by an earlier one.
pub fn default_edited_path(source: &Path) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "rom".into());
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| "sfc".into());
    let dir = source.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut n = 1;
    loop {
        let tag = if n == 1 {
            "edited".to_string()
        } else {
            format!("edited {n}")
        };
        let candidate = dir.join(format!("{stem} ({tag}).{ext}"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

static NEXT_SESSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Editor {
    /// Loads the ROM at `path` and opens room 1 (or room 0).
    pub fn open(path: &Path) -> Result<Editor, String> {
        let rom = Rom::load(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut editor = Editor::from_rom(rom, path.to_path_buf());
        let first = if editor.tables.room_count() > 1 { 1 } else { 0 };
        editor.select_room(first)?;
        Ok(editor)
    }

    pub fn from_rom(rom: Rom, source: PathBuf) -> Editor {
        let tables = Tables::load(&rom);
        let sprites = ObjectSprites::load(&rom);
        let mut editor = Editor {
            rom,
            tables,
            session: NEXT_SESSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            sprites,
            source,
            save_path: None,
            rooms: BTreeMap::new(),
            current: 0,
            tool: Tool::Select,
            tile: 0,
            new_object: (0x01, 0),
            selected_object: None,
            selected_door: None,
            group: None,
        };
        // The named levels are listed with their sizes, and the treasure
        // list covers every room of the cave: load them now.
        for index in editor.named_rooms() {
            let _ = editor.load_room(index);
        }
        editor
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
        self.tables.level_of(self.current)
    }

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

    /// The file the ROM was loaded from. Never written unless the user picks
    /// it in Save As.
    pub fn source_path(&self) -> &Path {
        &self.source
    }

    /// Where plain Save writes: the path of the last save, else the default
    /// `<name> (edited).sfc` next to the source.
    pub fn save_target(&self) -> PathBuf {
        self.save_path
            .clone()
            .unwrap_or_else(|| default_edited_path(&self.source))
    }

    // ---- rooms ----

    fn load_room(&mut self, index: usize) -> Result<(), String> {
        if self.rooms.contains_key(&index) {
            return Ok(());
        }
        let room = Room::load(&self.rom, &self.tables, index).map_err(|e| e.to_string())?;
        let tileset = Tileset::for_room(&self.rom, &self.tables, &room);
        self.rooms.insert(
            index,
            LoadedRoom {
                saved: MapSnapshot::of(&room),
                room,
                tileset,
                revision: 0,
                undo: Vec::new(),
                redo: Vec::new(),
                dirty: false,
            },
        );
        Ok(())
    }

    /// Makes `index` the current room, loading it if needed.
    pub fn select_room(&mut self, index: usize) -> Result<(), String> {
        self.end_group();
        self.load_room(index)?;
        if self.current != index {
            self.selected_object = None;
            self.selected_door = None;
        }
        self.current = index;
        Ok(())
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    pub fn loaded(&self, index: usize) -> Option<&LoadedRoom> {
        self.rooms.get(&index)
    }

    fn cur(&self) -> &LoadedRoom {
        self.rooms
            .get(&self.current)
            .expect("the current room is loaded")
    }

    fn cur_mut(&mut self) -> &mut LoadedRoom {
        self.rooms
            .get_mut(&self.current)
            .expect("the current room is loaded")
    }

    pub fn room(&self) -> &Room {
        &self.cur().room
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

    pub fn tileset(&self) -> &Tileset {
        &self.cur().tileset
    }

    /// Changes whenever the current room image must be re-rendered.
    pub fn revision(&self) -> u64 {
        self.cur().revision
    }

    pub fn is_dirty(&self, index: usize) -> bool {
        self.rooms.get(&index).is_some_and(|r| r.dirty)
    }

    pub fn dirty_rooms(&self) -> Vec<usize> {
        self.rooms
            .iter()
            .filter(|(_, r)| r.dirty)
            .map(|(i, _)| *i)
            .collect()
    }

    pub fn any_dirty(&self) -> bool {
        self.rooms.values().any(|r| r.dirty)
    }

    pub fn can_undo(&self) -> bool {
        !self.cur().undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.cur().redo.is_empty()
    }

    // ---- history ----

    /// Starts a gesture: every change until [`end_group`](Self::end_group)
    /// becomes one undo step.
    pub fn begin_group(&mut self) {
        if self.group.is_none() {
            self.group = Some(Vec::new());
        }
    }

    pub fn end_group(&mut self) {
        if let Some(group) = self.group.take()
            && !group.is_empty()
        {
            let lr = self.cur_mut();
            lr.undo.push(group);
        }
    }

    /// Applies `change` to the current room and records it.
    fn perform(&mut self, change: Change) {
        let in_group = self.group.is_some();
        let lr = self.cur_mut();
        change.apply(&mut lr.room);
        if change.alters_image() {
            lr.revision += 1;
        }
        lr.dirty = true;
        lr.redo.clear();
        if in_group {
            let group = self.group.as_mut().expect("checked");
            match group.last_mut() {
                Some(last) => {
                    if let Some(change) = last.merge(change) {
                        group.push(change);
                    }
                }
                None => group.push(change),
            }
        } else {
            self.cur_mut().undo.push(vec![change]);
        }
    }

    pub fn undo(&mut self) -> bool {
        self.end_group();
        let lr = self.cur_mut();
        let Some(entry) = lr.undo.pop() else {
            return false;
        };
        for change in entry.iter().rev() {
            change.revert(&mut lr.room);
            if change.alters_image() {
                lr.revision += 1;
            }
        }
        let reordered = entry.iter().any(Change::reorders_objects);
        let doors = entry.iter().any(Change::reorders_doors);
        lr.redo.push(entry);
        lr.dirty = true;
        if reordered {
            // The selected index may now name another object.
            self.selected_object = None;
        }
        if doors {
            self.selected_door = None;
        }
        self.fix_selection();
        true
    }

    pub fn redo(&mut self) -> bool {
        self.end_group();
        let lr = self.cur_mut();
        let Some(entry) = lr.redo.pop() else {
            return false;
        };
        for change in &entry {
            change.apply(&mut lr.room);
            if change.alters_image() {
                lr.revision += 1;
            }
        }
        let reordered = entry.iter().any(Change::reorders_objects);
        let doors = entry.iter().any(Change::reorders_doors);
        lr.undo.push(entry);
        lr.dirty = true;
        if reordered {
            self.selected_object = None;
        }
        if doors {
            self.selected_door = None;
        }
        self.fix_selection();
        true
    }

    fn fix_selection(&mut self) {
        let objects = self.room().objects.len();
        let doors = self.room().doors().len();
        if self.selected_object.is_some_and(|i| i >= objects) {
            self.selected_object = None;
        }
        if self.selected_door.is_some_and(|i| i >= doors) {
            self.selected_door = None;
        }
    }

    // ---- tiles ----

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        let r = self.room();
        x >= 0 && y >= 0 && (x as usize) < r.width && (y as usize) < r.height
    }

    pub fn tile_at(&self, x: i32, y: i32) -> Option<u16> {
        self.in_bounds(x, y)
            .then(|| self.room().tile(x as usize, y as usize))
    }

    /// Sets one cell to `id`. Returns whether it changed.
    pub fn set_tile(&mut self, x: i32, y: i32, id: u16) -> bool {
        let Some(old) = self.tile_at(x, y) else {
            return false;
        };
        if old == id {
            return false;
        }
        let index = y as usize * self.room().width + x as usize;
        self.perform(Change::Tiles(vec![TileChange {
            index,
            old,
            new: id,
        }]));
        true
    }

    /// Paints the current tile at a cell.
    pub fn paint(&mut self, x: i32, y: i32) -> bool {
        self.set_tile(x, y, self.tile)
    }

    /// Picks the tile under a cell as the current one.
    pub fn pick_tile(&mut self, x: i32, y: i32) -> bool {
        match self.tile_at(x, y) {
            Some(id) => {
                self.tile = id;
                true
            }
            None => false,
        }
    }

    /// Fills the cells between two corners (inclusive, any order) with the
    /// current tile. Returns how many cells changed.
    pub fn fill_rect(&mut self, a: (i32, i32), b: (i32, i32)) -> usize {
        let (x0, x1) = (a.0.min(b.0).max(0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1).max(0), a.1.max(b.1));
        let (width, height) = (self.room().width as i32, self.room().height as i32);
        let mut cells = Vec::new();
        for y in y0..=y1.min(height - 1) {
            for x in x0..=x1.min(width - 1) {
                let index = (y * width + x) as usize;
                let old = self.room().tiles[index];
                if old != self.tile {
                    cells.push(TileChange {
                        index,
                        old,
                        new: self.tile,
                    });
                }
            }
        }
        let n = cells.len();
        if n > 0 {
            self.perform(Change::Tiles(cells));
        }
        n
    }

    /// Replaces the 4-connected region of equal tiles around a cell with the
    /// current tile. Returns how many cells changed.
    pub fn flood_fill(&mut self, x: i32, y: i32) -> usize {
        let Some(target) = self.tile_at(x, y) else {
            return 0;
        };
        if target == self.tile {
            return 0;
        }
        let (width, height) = (self.room().width, self.room().height);
        let mut seen = vec![false; width * height];
        let mut cells = Vec::new();
        let mut stack = vec![(x as usize, y as usize)];
        while let Some((cx, cy)) = stack.pop() {
            let index = cy * width + cx;
            if seen[index] || self.room().tiles[index] != target {
                continue;
            }
            seen[index] = true;
            cells.push(TileChange {
                index,
                old: target,
                new: self.tile,
            });
            if cx > 0 {
                stack.push((cx - 1, cy));
            }
            if cx + 1 < width {
                stack.push((cx + 1, cy));
            }
            if cy > 0 {
                stack.push((cx, cy - 1));
            }
            if cy + 1 < height {
                stack.push((cx, cy + 1));
            }
        }
        let n = cells.len();
        self.perform(Change::Tiles(cells));
        n
    }

    // ---- objects ----

    fn clamp_position(&self, x: i32, y: i32) -> (u16, u16) {
        let max_x = (self.room().width * CELL).saturating_sub(1) as i32;
        let max_y = (self.room().height * CELL).saturating_sub(1) as i32;
        (x.clamp(0, max_x) as u16, y.clamp(0, max_y) as u16)
    }

    /// The object nearest to a room-pixel position within `radius` pixels.
    /// Later objects win ties (they are drawn on top).
    pub fn object_at(&self, x: i32, y: i32, radius: i32) -> Option<usize> {
        let mut best: Option<(usize, i32)> = None;
        for (i, o) in self.room().objects.iter().enumerate() {
            let (dx, dy) = ((o.x as i32 - x).abs(), (o.y as i32 - y).abs());
            if dx <= radius && dy <= radius {
                let d = dx.max(dy);
                if best.is_none_or(|(_, bd)| d <= bd) {
                    best = Some((i, d));
                }
            }
        }
        best.map(|(i, _)| i)
    }

    /// The door whose trigger box (the cell whose top-left corner is its
    /// position) contains a room-pixel position. Doors without a trigger have no box.
    pub fn door_at(&self, x: i32, y: i32) -> Option<usize> {
        // The position is the top-left corner of the door's cell: the
        // game's door stars sit 12 pixels right of and below it.
        let cell = CELL as i32;
        self.room().doors().iter().position(|d| {
            let (dx, dy) = (x - d.x as i32, y - d.y as i32);
            d.has_trigger() && (0..cell).contains(&dx) && (0..cell).contains(&dy)
        })
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
        let (x, y) = self.clamp_position(new.x as i32, new.y as i32);
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
        self.move_object(index, old.x as i32 + dx, old.y as i32 + dy)
    }

    // ---- doors, music, size ----

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
        let cell = CELL as u16;
        let (cx, cy) = (px - px % cell, py - py % cell);
        let here = self
            .level()
            .and_then(|l| Door::relative_to(l, self.current))?;
        let door = Door {
            dest_room: here,
            x: cx,
            y: cy,
            dest_x: cx + cell / 2,
            dest_y: cy + cell / 2,
        };
        let star = Object {
            variant: 0,
            id: DOOR_STAR,
            x: cx + cell / 2,
            y: cy + cell / 2,
        };
        self.insert_door_with(door, star)
    }

    /// Adds a warp star at a room-pixel position together with the door
    /// record it uses (one without a trigger area). Selects the door.
    pub fn add_warp(&mut self, x: i32, y: i32) -> Option<usize> {
        let (px, py) = self.clamp_position(x, y);
        let here = self
            .level()
            .and_then(|l| Door::relative_to(l, self.current))?;
        let door = Door {
            dest_room: here,
            x: Door::NO_TRIGGER,
            y: Door::NO_TRIGGER,
            dest_x: px,
            dest_y: py,
        };
        let star = Object {
            variant: self.room().doors().len().min(255) as u8,
            id: WARP_STAR,
            x: px,
            y: py,
        };
        self.insert_door_with(door, star)
    }

    fn insert_door_with(&mut self, door: Door, marker: Object) -> Option<usize> {
        let index = self.room().doors().len();
        if index >= MAX_DOORS {
            return None;
        }
        let outer = self.group.is_some();
        self.begin_group();
        self.perform(Change::InsertDoor { index, door });
        let at = self.room().objects.len();
        self.perform(Change::AddObject {
            index: at,
            object: marker,
        });
        if !outer {
            self.end_group();
        }
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
        let outer = self.group.is_some();
        self.begin_group();
        self.perform(Change::RemoveDoor { index, door });
        let half = CELL as u16 / 2;
        let star_at = (door.x.wrapping_add(half), door.y.wrapping_add(half));
        let marker = |o: &Object| {
            o.warp_door() == Some(index)
                || (o.id == DOOR_STAR && door.has_trigger() && (o.x, o.y) == star_at)
        };
        let doomed: Vec<usize> = (0..self.room().objects.len())
            .filter(|&i| marker(&self.room().objects[i]))
            .collect();
        for &i in doomed.iter().rev() {
            let object = self.room().objects[i];
            self.perform(Change::RemoveObject { index: i, object });
        }
        for i in 0..self.room().objects.len() {
            let old = self.room().objects[i];
            if old.warp_door().is_some_and(|d| d > index) {
                let new = Object {
                    variant: old.variant - 1,
                    ..old
                };
                self.perform(Change::Object { index: i, old, new });
            }
        }
        if !outer {
            self.end_group();
        }
        self.selected_door = None;
        self.selected_object = None;
        true
    }

    /// The warp stars of the current room that use door `index`.
    pub fn warps_of(&self, index: usize) -> Vec<usize> {
        let objects = &self.room().objects;
        (0..objects.len())
            .filter(|&i| objects[i].warp_door() == Some(index))
            .collect()
    }

    // ---- treasures ----

    /// Every treasure chest in the cave's rooms, edits included, by
    /// treasure number then room.
    pub fn chests(&self) -> Vec<ChestSpot> {
        let mut spots = Vec::new();
        for room in self.cave_rooms() {
            let Some(lr) = self.rooms.get(&room) else {
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

    pub fn set_music(&mut self, song: u8) -> bool {
        let old = self.room().music();
        if old == song {
            return false;
        }
        self.perform(Change::Music { old, new: song });
        true
    }

    /// Resizes the current room, keeping its top-left cells.
    pub fn resize(&mut self, width: usize, height: usize) -> bool {
        let (w, h) = (self.room().width, self.room().height);
        if width == 0 || height == 0 || (width, height) == (w, h) || width * height > MAX_CELLS {
            return false;
        }
        let old = MapSnapshot::of(self.room());
        let mut probe = self.room().clone();
        probe.resize(width, height);
        let new = MapSnapshot::of(&probe);
        self.perform(Change::Resize {
            old: Box::new(old),
            new: Box::new(new),
        });
        // Objects outside the new bounds stay; the game ignores them but the
        // user can see and delete them.
        true
    }

    // ---- saving ----

    /// Saves to [`save_target`](Self::save_target). Never the source file
    /// unless Save As chose it earlier.
    pub fn save(&mut self) -> Result<SaveSummary, String> {
        let target = self.save_target();
        self.write_to(target)
    }

    /// Saves to a path the user picked. This is the only way to overwrite
    /// the file the ROM was loaded from.
    pub fn save_as(&mut self, path: &Path) -> Result<SaveSummary, String> {
        self.write_to(path.to_path_buf())
    }

    fn write_to(&mut self, path: PathBuf) -> Result<SaveSummary, String> {
        self.end_group();
        let mut written = Vec::new();
        for index in self.dirty_rooms() {
            let lr = self.rooms.get_mut(&index).expect("dirty rooms are loaded");
            // Star and bomb blocks only break when the room lists their cell.
            let LoadedRoom {
                room,
                tileset,
                saved,
                ..
            } = lr;
            room.sync_blocks(
                |tile| tileset.is_breakable(tile),
                |x, y| saved.tile(x, y),
                &saved.blocks,
            );
            let report = room
                .save(&mut self.rom, &mut self.tables)
                .map_err(|e| format!("room {index}: {e}"))?;
            written.push((index, report));
        }
        self.rom
            .save(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        // Only now is the work safe: a failure above leaves every room
        // marked as changed, and the next save writes them all again.
        for &(index, _) in &written {
            let lr = self
                .rooms
                .get_mut(&index)
                .expect("written rooms are loaded");
            // Records dropped by an earlier save stay remembered, so undoing
            // back to a block restores what it breaks into.
            let mut blocks = lr.room.blocks();
            let forgotten =
                |old: &&kss_core::Block| !blocks.iter().any(|b| (b.x, b.y) == (old.x, old.y));
            let kept: Vec<_> = lr.saved.blocks.iter().filter(forgotten).copied().collect();
            blocks.extend(kept);
            lr.saved = MapSnapshot {
                blocks,
                ..MapSnapshot::of(&lr.room)
            };
            lr.dirty = false;
        }
        self.save_path = Some(path.clone());
        Ok(SaveSummary {
            path,
            rooms: written,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> Option<Editor> {
        let path = Rom::locate()?;
        Some(Editor::open(&path).expect("ROM opens"))
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kss-editor-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lines_have_no_gaps() {
        let cells = line_cells((0, 0), (5, 2));
        assert_eq!(cells.first(), Some(&(0, 0)));
        assert_eq!(cells.last(), Some(&(5, 2)));
        for pair in cells.windows(2) {
            assert!((pair[1].0 - pair[0].0).abs() <= 1 && (pair[1].1 - pair[0].1).abs() <= 1);
        }
        assert_eq!(line_cells((3, 3), (3, 3)), vec![(3, 3)]);
        assert_eq!(line_cells((2, 0), (0, 0)), vec![(2, 0), (1, 0), (0, 0)]);
    }

    #[test]
    fn default_path_is_next_to_source_and_differs() {
        let p = default_edited_path(Path::new("C:/roms/Kirby Super Star (USA).sfc"));
        assert_eq!(
            p,
            PathBuf::from("C:/roms/Kirby Super Star (USA) (edited).sfc")
        );
        // An existing file is never the default target.
        let dir = temp("default-path");
        let source = dir.join("game.sfc");
        assert_eq!(default_edited_path(&source), dir.join("game (edited).sfc"));
        std::fs::write(dir.join("game (edited).sfc"), b"earlier session").unwrap();
        assert_eq!(
            default_edited_path(&source),
            dir.join("game (edited 2).sfc")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn opens_green_greens_room_one() {
        let Some(e) = editor() else { return };
        assert_eq!(e.current_index(), 1);
        assert_eq!((e.room().width, e.room().height), (100, 8));
        let named = e.named_rooms();
        assert_eq!(named.len(), 27 + 81);
        assert_eq!(e.cave_rooms(), 72..153);
        assert!(e.loaded(26).is_some() && e.loaded(152).is_some());
        assert!(e.loaded(27).is_none());
        assert!(!e.any_dirty());
    }

    #[test]
    fn paint_is_one_undo_step_per_stroke() {
        let Some(mut e) = editor() else { return };
        e.tile = 0x0C;
        let before = e.room().tiles.clone();
        e.begin_group();
        for x in 0..5 {
            e.paint(x, 0);
        }
        e.end_group();
        assert!((0..5).all(|x| e.room().tile(x, 0) == 0x0C));
        assert!(e.is_dirty(1));
        assert!(e.undo());
        assert_eq!(e.room().tiles, before);
        assert!(!e.can_undo());
        assert!(e.redo());
        assert!((0..5).all(|x| e.room().tile(x, 0) == 0x0C));
    }

    #[test]
    fn fill_rect_and_flood_fill_undo() {
        let Some(mut e) = editor() else { return };
        let before = e.room().tiles.clone();
        e.tile = 0x0C;
        assert!(e.fill_rect((2, 1), (0, 0)) > 0);
        for (x, y) in [(0, 0), (1, 0), (2, 0), (0, 1), (2, 1)] {
            assert_eq!(e.room().tile(x, y), 0x0C);
        }
        e.undo();
        assert_eq!(e.room().tiles, before);
        // Flood fill the sky region around (50, 0) with another tile.
        let sky = e.room().tile(50, 0);
        e.tile = if sky == 0x0D { 0x0E } else { 0x0D };
        let n = e.flood_fill(50, 0);
        assert!(n > 0);
        assert!(e.room().tiles.iter().filter(|&&t| t == e.tile).count() >= n);
        assert_eq!(e.room().tile(50, 0), e.tile);
        e.undo();
        assert_eq!(e.room().tiles, before);
        // Filling with the tile already there changes nothing.
        e.tile = e.room().tile(50, 0);
        assert_eq!(e.flood_fill(50, 0), 0);
        assert!(!e.can_undo());
    }

    #[test]
    fn objects_add_move_remove_undo() {
        let Some(mut e) = editor() else { return };
        let before = e.room().objects.clone();
        e.new_object = (0x02, 3);
        let i = e.add_object(100, 50);
        assert_eq!(
            e.room().objects[i],
            Object {
                id: 2,
                variant: 3,
                x: 100,
                y: 50
            }
        );
        assert_eq!(e.selected_object, Some(i));
        assert_eq!(e.object_at(104, 47, 8), Some(i));
        e.begin_group();
        e.move_object(i, 110, 60);
        e.move_object(i, 120, 70);
        e.nudge_object(i, 1, 0);
        e.end_group();
        assert_eq!((e.room().objects[i].x, e.room().objects[i].y), (121, 70));
        // The whole drag is one step: one undo goes back to the placement.
        e.undo();
        assert_eq!((e.room().objects[i].x, e.room().objects[i].y), (100, 50));
        e.redo();
        assert!(e.remove_object(i));
        assert_eq!(e.selected_object, None);
        e.undo();
        e.undo();
        e.undo();
        assert_eq!(e.room().objects, before);
        // Positions clamp to the room.
        let i = e.add_object(-5, 100_000);
        assert_eq!(
            (e.room().objects[i].x, e.room().objects[i].y),
            (0, (8 * CELL - 1) as u16)
        );
    }

    #[test]
    fn doors_music_and_resize_undo() {
        let Some(mut e) = editor() else { return };
        let doors = e.room().doors();
        if let Some(&door) = doors.first() {
            let moved = Door {
                dest_x: door.dest_x.wrapping_add(8),
                ..door
            };
            assert!(e.set_door(0, moved));
            assert_eq!(e.room().doors()[0], moved);
            e.undo();
            assert_eq!(e.room().doors()[0], door);
        }
        let music = e.room().music();
        assert!(e.set_music(music ^ 1));
        assert!(!e.set_music(music ^ 1));
        e.undo();
        assert_eq!(e.room().music(), music);
        let before = (e.room().width, e.room().height, e.room().tiles.clone());
        let rev = e.revision();
        assert!(e.resize(90, 9));
        assert_eq!((e.room().width, e.room().height), (90, 9));
        assert_eq!(e.room().tiles.len(), 90 * 9);
        assert!(e.revision() > rev);
        e.undo();
        assert_eq!(
            (e.room().width, e.room().height, e.room().tiles.clone()),
            before
        );
    }

    #[test]
    fn edits_survive_a_save_and_the_original_is_untouched() {
        let Some(path) = Rom::locate() else { return };
        let original = std::fs::read(&path).unwrap();
        let dir = temp("save");
        // Work on a copy so the test cannot touch the real ROM even by accident.
        let source = dir.join("Kirby Super Star (USA).sfc");
        std::fs::write(&source, &original).unwrap();

        let mut e = Editor::open(&source).unwrap();
        e.tile = 0x0C;
        let old_tile = e.room().tile(3, 2);
        assert_ne!(old_tile, 0x0C, "pick a cell that actually changes");
        assert!(e.set_tile(3, 2, 0x0C));
        e.new_object = (0x1A, 1);
        let added = e.add_object(250, 77);
        let first = e.room().objects[0];
        e.move_object(0, first.x as i32 + 2, first.y as i32 + 3);
        let expected_objects = e.room().objects.clone();

        // Plain Save defaults to a new file next to the source.
        let summary = e.save().unwrap();
        assert_eq!(
            summary.path,
            dir.join("Kirby Super Star (USA) (edited).sfc")
        );
        assert_ne!(summary.path, source);
        assert_eq!(summary.rooms.len(), 1);
        assert!(!e.any_dirty());
        assert_eq!(
            std::fs::read(&source).unwrap(),
            original,
            "the source ROM must be untouched"
        );

        let reloaded = Editor::open(&summary.path).unwrap();
        assert_eq!(reloaded.room().tile(3, 2), 0x0C);
        assert_eq!(reloaded.room().objects, expected_objects);
        assert_eq!(reloaded.room().objects[added].id, 0x1A);

        // After the first save, Save keeps writing to the edited file.
        assert_eq!(e.save_target(), summary.path);
        e.set_tile(4, 2, 0x0C);
        let again = e.save().unwrap();
        assert_eq!(again.path, summary.path);
        assert_eq!(std::fs::read(&source).unwrap(), original);

        // Save As the source is explicit and allowed.
        e.save_as(&source).unwrap();
        assert_ne!(std::fs::read(&source).unwrap(), original);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resized_room_round_trips() {
        let Some(path) = Rom::locate() else { return };
        let dir = temp("resize");
        let source = dir.join("rom.sfc");
        std::fs::copy(&path, &source).unwrap();
        let mut e = Editor::open(&source).unwrap();
        e.tile = 0x0C;
        e.resize(104, 8);
        e.set_tile(103, 7, 0x0C);
        let summary = e.save().unwrap();
        let r = Editor::open(&summary.path).unwrap();
        assert_eq!((r.room().width, r.room().height), (104, 8));
        assert_eq!(r.room().tile(103, 7), 0x0C);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn switching_rooms_keeps_each_history() {
        let Some(mut e) = editor() else { return };
        e.tile = 0x0C;
        e.set_tile(0, 0, 0x0C);
        e.select_room(2).unwrap();
        assert!(!e.can_undo());
        e.select_room(1).unwrap();
        assert!(e.can_undo());
        assert_eq!(e.dirty_rooms(), vec![1]);
    }

    #[test]
    fn a_failed_write_keeps_the_rooms_dirty() {
        let Some(mut e) = editor() else { return };
        e.tile = 0x00C;
        e.paint(40, 1);
        let missing = temp("failed-write").join("no-such-folder").join("out.sfc");
        assert!(e.save_as(&missing).is_err());
        assert!(e.any_dirty(), "the edit is not on disk");
        // The next save writes it.
        let good = temp("failed-write").join("out.sfc");
        let summary = e.save_as(&good).unwrap();
        assert_eq!(summary.rooms.len(), 1);
        assert!(!e.any_dirty());
        let reopened = Editor::open(&good).unwrap();
        assert_eq!(reopened.tile_at(40, 1), Some(0x00C));
        let _ = std::fs::remove_dir_all(temp("failed-write"));
    }

    #[test]
    fn resizing_keeps_breakable_blocks_on_their_cells() {
        let Some(mut e) = editor() else { return };
        // Room 3: star blocks at (19..24, 4), their caps in the row above.
        e.select_room(3).unwrap();
        let blocks = e.room().blocks();
        assert_eq!(blocks.len(), 8);
        assert!(e.resize(40, 8));
        assert_eq!(e.room().blocks(), blocks);
        let out = temp("resize-blocks").join("out.sfc");
        e.save_as(&out).unwrap();
        let mut reopened = Editor::open(&out).unwrap();
        reopened.select_room(3).unwrap();
        assert_eq!((reopened.room().width, reopened.room().height), (40, 8));
        assert_eq!(reopened.room().blocks(), blocks);
        // Undo restores the size and the list.
        assert!(e.undo());
        assert_eq!((e.room().width, e.room().blocks()), (37, blocks));
        // A room that could not be stored is refused.
        assert!(!e.resize(400, 100));
        let _ = std::fs::remove_dir_all(temp("resize-blocks"));
    }

    #[test]
    fn a_block_restored_by_undo_after_a_save_keeps_its_record() {
        let Some(mut e) = editor() else { return };
        e.select_room(3).unwrap();
        let blocks = e.room().blocks();
        let dir = temp("undo-block");
        e.tile = 0;
        e.paint(19, 4);
        e.save_as(&dir.join("a.sfc")).unwrap();
        assert_eq!(e.room().blocks().len(), 7);
        assert!(e.undo());
        e.save_as(&dir.join("b.sfc")).unwrap();
        assert_eq!(e.room().blocks(), blocks);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_cave_lists_its_treasure_chests() {
        let Some(e) = editor() else { return };
        let chests = e.chests();
        // 58 chests: every treasure but 0 and 16, once each.
        assert_eq!(chests.len(), 58);
        let numbers: Vec<u8> = chests.iter().map(|c| c.treasure).collect();
        let expected: Vec<u8> = (0..60).filter(|n| *n != 0 && *n != 16).collect();
        assert_eq!(numbers, expected);
        let coin = chests.iter().find(|c| c.treasure == 1).unwrap();
        assert_eq!((coin.room, coin.x, coin.y), (80, 204, 124));
        assert_eq!(
            e.loaded(80).unwrap().room.objects[coin.object].id,
            TREASURE_CHEST
        );
    }

    #[test]
    fn labels_resolve_doors_chests_and_warps() {
        let Some(mut e) = editor() else { return };
        e.select_room(7).unwrap();
        let door = e.room().doors()[0];
        assert_eq!(e.destination_label(&door), "room 8");
        e.select_room(80).unwrap();
        let chest = *e
            .room()
            .objects
            .iter()
            .find(|o| o.id == TREASURE_CHEST)
            .unwrap();
        assert_eq!(e.object_label(&chest), "Treasure 1: Gold Coin");
        e.select_room(134).unwrap();
        let warp = *e.room().objects.iter().find(|o| o.id == WARP_STAR).unwrap();
        assert_eq!(e.object_label(&warp), "Warp Star \u{2192} room 148");
    }

    #[test]
    fn doors_and_warps_add_and_remove_in_one_step() {
        let Some(mut e) = editor() else { return };
        // Room 1: door 0 (with its door star) and door 1, used by the warp star.
        let (doors, objects) = (e.room().doors(), e.room().objects.clone());
        let i = e.add_door(100, 130).unwrap();
        assert_eq!(i, 2);
        assert_eq!(e.selected_door, Some(2));
        let door = e.room().doors()[2];
        assert_eq!((door.x, door.y, door.dest_room), (96, 120, 1));
        let star = *e.room().objects.last().unwrap();
        assert_eq!((star.id, star.x, star.y), (DOOR_STAR, 108, 132));
        assert!(e.undo());
        assert_eq!(e.room().doors(), doors);
        assert_eq!(e.room().objects, objects);
        assert!(e.redo());
        // A warp added after it uses door 3.
        let w = e.add_warp(300, 60).unwrap();
        assert_eq!(w, 3);
        assert!(!e.room().doors()[3].has_trigger());
        assert_eq!(e.warps_of(3).len(), 1);
        // Deleting door 0 takes its door star and renumbers both warp stars.
        assert!(e.remove_door(0));
        assert_eq!(e.room().doors().len(), 3);
        let warps = |e: &Editor| -> Vec<usize> {
            e.room()
                .objects
                .iter()
                .filter_map(|o| o.warp_door())
                .collect()
        };
        assert_eq!(warps(&e), vec![0, 2]);
        assert!(
            !e.room()
                .objects
                .iter()
                .any(|o| o.id == DOOR_STAR && (o.x, o.y) == (396, 132))
        );
        // Deleting the warp's door takes the warp star.
        assert!(e.remove_door(2));
        assert!(e.warps_of(2).is_empty());
        assert_eq!(warps(&e), vec![0]);
        // Each deletion is one undo step.
        assert!(e.undo());
        assert!(e.undo());
        assert_eq!(warps(&e), vec![1, 3]);
        assert!(e.undo());
        assert!(e.undo());
        assert_eq!(e.room().doors(), doors);
        assert_eq!(e.room().objects, objects);
        // FFFF would end the list.
        let first = e.room().doors()[0];
        assert!(!e.set_door(
            0,
            Door {
                dest_room: 0xFFFF,
                ..first
            }
        ));
    }

    #[test]
    fn an_added_door_survives_a_save() {
        let Some(mut e) = editor() else { return };
        e.select_room(75).unwrap();
        let blocks = e.room().blocks();
        let i = e.add_door(500, 300).unwrap();
        let door = Door {
            dest_room: 8,
            ..e.room().doors()[i]
        };
        assert!(e.set_door(i, door));
        let out = temp("door-save").join("out.sfc");
        e.save_as(&out).unwrap();
        let mut back = Editor::open(&out).unwrap();
        back.select_room(75).unwrap();
        assert_eq!(back.room().doors()[i], door);
        assert_eq!(back.destination_label(&door), "room 80");
        assert_eq!(back.room().blocks(), blocks);
        assert_eq!(back.chests().len(), 58);
        let _ = std::fs::remove_dir_all(temp("door-save"));
    }

    #[test]
    fn undoing_a_delete_drops_the_selection() {
        let Some(mut e) = editor() else { return };
        assert!(e.remove_object(0));
        e.selected_object = Some(0);
        assert!(e.undo());
        assert_eq!(e.selected_object, None);
    }

    #[test]
    fn each_opened_rom_has_its_own_session() {
        let (Some(a), Some(b)) = (editor(), editor()) else {
            return;
        };
        assert_ne!(a.session(), b.session());
        assert_eq!(
            (a.current_index(), a.revision()),
            (b.current_index(), b.revision())
        );
    }
}
