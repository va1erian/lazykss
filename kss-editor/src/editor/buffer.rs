//! The scratch buffer around the map, and the stamp copied from either.
//!
//! The buffer is a band of cells around the room where motifs are composed.
//! It is undoable like the map but never saved and never marks the room as
//! modified. The stamp tool copies a rectangle of cells (from the buffer or
//! the map) and pastes it anywhere on either.

use std::collections::HashSet;

use super::Editor;
use crate::history::{Buffer, BufferChange, Change, TileChange};

/// How far the scratch buffer reaches around a room, in cells.
pub const BUFFER_MARGIN: i32 = 16;

/// What a cell position holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// A cell of the room's map.
    Room(u16),
    /// A cell of the scratch buffer around the map, maybe empty.
    Buffer(Option<u16>),
    /// Beyond the buffer.
    Outside,
}

impl Cell {
    /// The tile shown there, if any.
    pub fn tile(self) -> Option<u16> {
        match self {
            Cell::Room(id) => Some(id),
            Cell::Buffer(id) => id,
            Cell::Outside => None,
        }
    }
}

/// A rectangle of cells copied by the stamp tool, row-major; `None` cells
/// (empty buffer cells) leave what is under them when pasted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub width: i32,
    pub height: i32,
    pub cells: Vec<Option<u16>>,
}

impl Stamp {
    /// The cell at an offset from the stamp's top-left corner.
    pub fn get(&self, dx: i32, dy: i32) -> Option<u16> {
        if dx < 0 || dy < 0 || dx >= self.width || dy >= self.height {
            return None;
        }
        let index = usize::try_from(dy * self.width + dx).ok()?;
        self.cells.get(index).copied().flatten()
    }

    /// Every filled cell, as an offset from the top-left corner and a tile.
    pub fn filled(&self) -> impl Iterator<Item = ((i32, i32), u16)> + use<'_> {
        (0..self.height)
            .flat_map(move |dy| (0..self.width).map(move |dx| (dx, dy)))
            .filter_map(|(dx, dy)| self.get(dx, dy).map(|id| ((dx, dy), id)))
    }
}

impl Editor {
    /// The cells the map and its buffer cover: `(left, top, right, bottom)`,
    /// right and bottom exclusive.
    pub fn canvas_cells(&self) -> (i32, i32, i32, i32) {
        let room = self.room();
        let w = i32::try_from(room.width).unwrap_or(i32::MAX);
        let h = i32::try_from(room.height).unwrap_or(i32::MAX);
        (
            -BUFFER_MARGIN,
            -BUFFER_MARGIN,
            w.saturating_add(BUFFER_MARGIN),
            h.saturating_add(BUFFER_MARGIN),
        )
    }

    /// What a cell position holds: the map, the buffer around it, or nothing.
    pub fn cell(&self, x: i32, y: i32) -> Cell {
        if let Some(id) = self.tile_at(x, y) {
            return Cell::Room(id);
        }
        let (l, t, r, b) = self.canvas_cells();
        if x >= l && y >= t && x < r && y < b {
            Cell::Buffer(self.buffer().get(&(x, y)).copied())
        } else {
            Cell::Outside
        }
    }

    /// The scratch buffer of the current room. Cells now under the map (the
    /// room grew) are kept but hidden.
    pub fn buffer(&self) -> &Buffer {
        &self.rooms.active().buffer
    }

    /// Sets cells of the map or the buffer (`None` empties a buffer cell; a
    /// map cell cannot be emptied), as one undo step. Returns how many
    /// changed.
    pub fn set_cells(
        &mut self,
        cells: impl IntoIterator<Item = ((i32, i32), Option<u16>)>,
    ) -> usize {
        let mut tiles = Vec::new();
        let mut buffer = Vec::new();
        let mut seen = HashSet::new();
        for ((x, y), new) in cells {
            if !seen.insert((x, y)) {
                continue;
            }
            match (self.cell(x, y), self.cell_index(x, y)) {
                (Cell::Room(old), Some(index)) => {
                    if let Some(new) = new.filter(|&n| n != old) {
                        tiles.push(TileChange { index, old, new });
                    }
                }
                (Cell::Buffer(old), _) if old != new => buffer.push(BufferChange {
                    cell: (x, y),
                    old,
                    new,
                }),
                _ => {}
            }
        }
        let n = tiles.len() + buffer.len();
        if n > 0 {
            self.as_one_step(|e| {
                if !tiles.is_empty() {
                    e.perform(Change::Tiles(tiles));
                }
                if !buffer.is_empty() {
                    e.perform(Change::Buffer(buffer));
                }
            });
        }
        n
    }

    /// Empties a buffer cell. Map cells are left alone.
    pub fn erase(&mut self, x: i32, y: i32) -> bool {
        self.set_cells([((x, y), None)]) > 0
    }

    /// Copies the cells between two corners (inclusive, any order), from the
    /// map and the buffer, as the stamp. Returns its size, or `None` (keeping
    /// the old stamp) when every cell is empty.
    pub fn copy_stamp(&mut self, a: (i32, i32), b: (i32, i32)) -> Option<(i32, i32)> {
        let (l, t, r, bottom) = self.canvas_cells();
        // Only the map and buffer hold cells: clip the corners to them.
        let (x0, x1) = (a.0.min(b.0).max(l), a.0.max(b.0).min(r - 1));
        let (y0, y1) = (a.1.min(b.1).max(t), a.1.max(b.1).min(bottom - 1));
        if x0 > x1 || y0 > y1 {
            return None;
        }
        let (width, height) = (x1 - x0 + 1, y1 - y0 + 1);
        let cells: Vec<Option<u16>> = (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
            .map(|(x, y)| self.cell(x, y).tile())
            .collect();
        if cells.iter().all(Option::is_none) {
            return None;
        }
        self.stamp = Some(Stamp {
            width,
            height,
            cells,
        });
        Some((width, height))
    }

    /// Pastes the stamp with its top-left corner at a cell, on the map and
    /// in the buffer. Empty stamp cells leave what is there. Returns how
    /// many cells changed.
    pub fn paste_stamp(&mut self, at: (i32, i32)) -> usize {
        let Some(stamp) = &self.stamp else {
            return 0;
        };
        let cells: Vec<_> = stamp
            .filled()
            .map(|((dx, dy), id)| ((at.0.saturating_add(dx), at.1.saturating_add(dy)), Some(id)))
            .collect();
        self.set_cells(cells)
    }
}
