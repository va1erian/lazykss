//! Writing the edited rooms to a ROM file, and where plain Save writes.

use std::path::{Path, PathBuf};

use kss_core::{Block, SaveReport};

use super::{Editor, LoadedRoom};
use crate::history::MapSnapshot;

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
    let candidate = |n: u32| {
        let tag = if n == 1 {
            "edited".to_string()
        } else {
            format!("edited {n}")
        };
        dir.join(format!("{stem} ({tag}).{ext}"))
    };
    // Four billion earlier saves cannot all exist; the last name only stands
    // in so the search has an end.
    (1..u32::MAX)
        .map(candidate)
        .find(|path| !path.exists())
        .unwrap_or_else(|| candidate(u32::MAX))
}

impl LoadedRoom {
    /// Records that the room as it is now is on disk.
    fn mark_saved(&mut self) {
        // Records dropped by an earlier save stay remembered, so undoing
        // back to a block restores what it breaks into.
        let mut blocks = self.room.blocks();
        let forgotten = |old: &&Block| !blocks.iter().any(|b| (b.x, b.y) == (old.x, old.y));
        let kept: Vec<Block> = self
            .saved
            .blocks
            .iter()
            .filter(forgotten)
            .copied()
            .collect();
        blocks.extend(kept);
        self.saved = MapSnapshot {
            blocks,
            ..MapSnapshot::of(&self.room)
        };
        self.dirty = false;
    }
}

impl Editor {
    /// Where plain Save writes: the path of the last save, else the default
    /// `<name> (edited).sfc` next to the source.
    pub fn save_target(&self) -> PathBuf {
        self.save_path
            .clone()
            .unwrap_or_else(|| default_edited_path(&self.source))
    }

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
            let Some(lr) = self.rooms.get_mut(index) else {
                continue;
            };
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
            if let Some(lr) = self.rooms.get_mut(index) {
                lr.mark_saved();
            }
        }
        self.save_path = Some(path.clone());
        Ok(SaveSummary {
            path,
            rooms: written,
        })
    }
}
