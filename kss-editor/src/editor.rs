//! The editing model: a ROM, its loaded rooms and everything the UI changes.
//!
//! Nothing here knows about windows. The UI turns mouse and keyboard input
//! into calls on [`Editor`]; every call that changes a room is undoable and
//! marks the room as modified.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kss_core::room::SPRING_BREEZE_LEVELS;
use kss_core::{CELL, Door, Object, ObjectSprites, Rom, Room, SaveReport, Tables, Tileset};

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
    pub const ALL: [Tool; 5] = [Tool::Select, Tool::Paint, Tool::Rect, Tool::Fill, Tool::AddObject];

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
    /// Bumped whenever the room image changes, to invalidate renders.
    pub revision: u64,
    pub undo: Vec<Entry>,
    pub redo: Vec<Entry>,
    pub dirty: bool,
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
        let name = self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if self.rooms.is_empty() {
            return format!("Saved {name} (no room changes)");
        }
        let rooms: Vec<String> = self
            .rooms
            .iter()
            .map(|(room, r)| {
                let moved = if r.moved { ", moved" } else { "" };
                format!("room {room}: {} bytes (was {}{moved})", r.len, r.previous_len)
            })
            .collect();
        format!("Saved {name}: {}", rooms.join("; "))
    }
}

pub struct Editor {
    rom: Rom,
    tables: Tables,
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
    let (sx, sy) = (if a.0 < b.0 { 1 } else { -1 }, if a.1 < b.1 { 1 } else { -1 });
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

/// `<name> (edited).sfc` next to `source`, never `source` itself.
pub fn default_edited_path(source: &Path) -> PathBuf {
    let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "rom".into());
    let ext = source.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_else(|| "sfc".into());
    let dir = source.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut n = 1;
    loop {
        let tag = if n == 1 { "edited".to_string() } else { format!("edited {n}") };
        let candidate = dir.join(format!("{stem} ({tag}).{ext}"));
        if candidate != source {
            return candidate;
        }
        n += 1;
    }
}

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
        // The Spring Breeze rooms are listed with their sizes: load them now.
        for index in editor.spring_breeze_rooms() {
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

    pub fn spring_breeze_rooms(&self) -> std::ops::Range<usize> {
        let start = self.tables.levels[SPRING_BREEZE_LEVELS.start].rooms.start;
        let end = self.tables.levels[SPRING_BREEZE_LEVELS.end - 1].rooms.end;
        start..end.min(self.tables.room_count())
    }

    /// The file the ROM was loaded from. Never written unless the user picks
    /// it in Save As.
    pub fn source_path(&self) -> &Path {
        &self.source
    }

    /// Where plain Save writes: the path of the last save, else the default
    /// `<name> (edited).sfc` next to the source.
    pub fn save_target(&self) -> PathBuf {
        self.save_path.clone().unwrap_or_else(|| default_edited_path(&self.source))
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
            LoadedRoom { room, tileset, revision: 0, undo: Vec::new(), redo: Vec::new(), dirty: false },
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
        self.rooms.get(&self.current).expect("the current room is loaded")
    }

    fn cur_mut(&mut self) -> &mut LoadedRoom {
        self.rooms.get_mut(&self.current).expect("the current room is loaded")
    }

    pub fn room(&self) -> &Room {
        &self.cur().room
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
        self.rooms.iter().filter(|(_, r)| r.dirty).map(|(i, _)| *i).collect()
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
        if let Some(group) = self.group.take() {
            if !group.is_empty() {
                let lr = self.cur_mut();
                lr.undo.push(group);
            }
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
        let Some(entry) = lr.undo.pop() else { return false };
        for change in entry.iter().rev() {
            change.revert(&mut lr.room);
            if change.alters_image() {
                lr.revision += 1;
            }
        }
        lr.redo.push(entry);
        lr.dirty = true;
        self.fix_selection();
        true
    }

    pub fn redo(&mut self) -> bool {
        self.end_group();
        let lr = self.cur_mut();
        let Some(entry) = lr.redo.pop() else { return false };
        for change in &entry {
            change.apply(&mut lr.room);
            if change.alters_image() {
                lr.revision += 1;
            }
        }
        lr.undo.push(entry);
        lr.dirty = true;
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
        self.in_bounds(x, y).then(|| self.room().tile(x as usize, y as usize))
    }

    /// Sets one cell to `id`. Returns whether it changed.
    pub fn set_tile(&mut self, x: i32, y: i32, id: u16) -> bool {
        let Some(old) = self.tile_at(x, y) else { return false };
        if old == id {
            return false;
        }
        let index = y as usize * self.room().width + x as usize;
        self.perform(Change::Tiles(vec![TileChange { index, old, new: id }]));
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
                    cells.push(TileChange { index, old, new: self.tile });
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
        let Some(target) = self.tile_at(x, y) else { return 0 };
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
            cells.push(TileChange { index, old: target, new: self.tile });
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

    /// The door whose trigger box (one cell, centred on its position)
    /// contains a room-pixel position. Doors without a trigger have no box.
    pub fn door_at(&self, x: i32, y: i32) -> Option<usize> {
        let half = CELL as i32 / 2;
        self.room().doors().iter().position(|d| {
            d.has_trigger() && (d.x as i32 - x).abs() <= half && (d.y as i32 - y).abs() <= half
        })
    }

    /// Places an object of the current `new_object` kind and selects it.
    pub fn add_object(&mut self, x: i32, y: i32) -> usize {
        let (px, py) = self.clamp_position(x, y);
        let object = Object { id: self.new_object.0, variant: self.new_object.1, x: px, y: py };
        let index = self.room().objects.len();
        self.perform(Change::AddObject { index, object });
        self.selected_object = Some(index);
        index
    }

    pub fn remove_object(&mut self, index: usize) -> bool {
        let Some(object) = self.room().objects.get(index).copied() else { return false };
        self.perform(Change::RemoveObject { index, object });
        self.selected_object = None;
        true
    }

    /// Replaces object `index`. A no-op when nothing differs.
    pub fn set_object(&mut self, index: usize, new: Object) -> bool {
        let Some(old) = self.room().objects.get(index).copied() else { return false };
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
        let Some(old) = self.room().objects.get(index).copied() else { return false };
        let (x, y) = self.clamp_position(x, y);
        self.set_object(index, Object { x, y, ..old })
    }

    pub fn nudge_object(&mut self, index: usize, dx: i32, dy: i32) -> bool {
        let Some(old) = self.room().objects.get(index).copied() else { return false };
        self.move_object(index, old.x as i32 + dx, old.y as i32 + dy)
    }

    // ---- doors, music, size ----

    pub fn set_door(&mut self, index: usize, new: Door) -> bool {
        let doors = self.room().doors();
        let Some(old) = doors.get(index).copied() else { return false };
        if old == new {
            return false;
        }
        self.perform(Change::Door { index, old, new });
        true
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
        if width == 0 || height == 0 || (width, height) == (w, h) {
            return false;
        }
        let old = MapSnapshot::of(self.room());
        let mut probe = self.room().clone();
        probe.resize(width, height);
        let new = MapSnapshot::of(&probe);
        self.perform(Change::Resize { old: Box::new(old), new: Box::new(new) });
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
        let dirty = self.dirty_rooms();
        for index in dirty {
            let lr = self.rooms.get_mut(&index).expect("dirty rooms are loaded");
            let report = lr.room.save(&mut self.rom, &mut self.tables).map_err(|e| format!("room {index}: {e}"))?;
            lr.dirty = false;
            written.push((index, report));
        }
        self.rom.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.save_path = Some(path.clone());
        Ok(SaveSummary { path, rooms: written })
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
        let dir = std::env::temp_dir().join(format!("kss-editor-test-{}-{name}", std::process::id()));
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
        assert_eq!(p, PathBuf::from("C:/roms/Kirby Super Star (USA) (edited).sfc"));
        let q = default_edited_path(Path::new("C:/roms/Kirby Super Star (USA) (edited).sfc"));
        assert_ne!(q, PathBuf::from("C:/roms/Kirby Super Star (USA) (edited).sfc"));
    }

    #[test]
    fn opens_green_greens_room_one() {
        let Some(e) = editor() else { return };
        assert_eq!(e.current_index(), 1);
        assert_eq!((e.room().width, e.room().height), (100, 8));
        assert_eq!(e.spring_breeze_rooms(), 0..27);
        assert!(e.loaded(26).is_some());
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
        assert_eq!(e.room().objects[i], Object { id: 2, variant: 3, x: 100, y: 50 });
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
        assert_eq!((e.room().objects[i].x, e.room().objects[i].y), (0, (8 * CELL - 1) as u16));
    }

    #[test]
    fn doors_music_and_resize_undo() {
        let Some(mut e) = editor() else { return };
        let doors = e.room().doors();
        if let Some(&door) = doors.first() {
            let moved = Door { dest_x: door.dest_x.wrapping_add(8), ..door };
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
        assert_eq!((e.room().width, e.room().height, e.room().tiles.clone()), before);
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
        assert_eq!(summary.path, dir.join("Kirby Super Star (USA) (edited).sfc"));
        assert_ne!(summary.path, source);
        assert_eq!(summary.rooms.len(), 1);
        assert!(!e.any_dirty());
        assert_eq!(std::fs::read(&source).unwrap(), original, "the source ROM must be untouched");

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
}
