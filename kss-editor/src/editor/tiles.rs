//! Painting and filling the tile map of the current room.

use super::Editor;
use crate::history::{Change, TileChange};

/// The cells on the line between two cells, both included, so a fast drag
/// paints without gaps.
pub fn line_cells(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    // Bresenham's line, worked in i64: the ends come from the mouse, and
    // their distance (and twice the error term) can exceed an i32.
    let (ax, ay) = (i64::from(a.0), i64::from(a.1));
    let (bx, by) = (i64::from(b.0), i64::from(b.1));
    let (dx, dy) = ((bx - ax).abs(), -(by - ay).abs());
    let (sx, sy) = (if ax < bx { 1 } else { -1 }, if ay < by { 1 } else { -1 });
    let (mut x, mut y, mut err) = (ax, ay, dx + dy);
    let mut cells = vec![a];
    while (x, y) != (bx, by) {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        // Every point lies between the two ends, so it fits an i32 again.
        let (Ok(cx), Ok(cy)) = (i32::try_from(x), i32::try_from(y)) else {
            break;
        };
        cells.push((cx, cy));
    }
    cells
}

impl Editor {
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        self.cell_index(x, y).is_some()
    }

    /// The index in the current room's tiles of a cell, `None` outside the
    /// room.
    fn cell_index(&self, x: i32, y: i32) -> Option<usize> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        let r = self.room();
        (x < r.width && y < r.height).then(|| y * r.width + x)
    }

    pub fn tile_at(&self, x: i32, y: i32) -> Option<u16> {
        let index = self.cell_index(x, y)?;
        self.room().tiles.get(index).copied()
    }

    /// Sets one cell to `id`. Returns whether it changed.
    pub fn set_tile(&mut self, x: i32, y: i32, id: u16) -> bool {
        let (Some(index), Some(old)) = (self.cell_index(x, y), self.tile_at(x, y)) else {
            return false;
        };
        if old == id {
            return false;
        }
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
        let room = self.room();
        let width = i32::try_from(room.width).unwrap_or(i32::MAX);
        let height = i32::try_from(room.height).unwrap_or(i32::MAX);
        let (x0, x1) = (a.0.min(b.0).max(0), a.0.max(b.0).min(width - 1));
        let (y0, y1) = (a.1.min(b.1).max(0), a.1.max(b.1).min(height - 1));
        let mut cells = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                let Some(index) = self.cell_index(x, y) else {
                    continue;
                };
                match room.tiles.get(index) {
                    Some(&old) if old != self.tile => cells.push(TileChange {
                        index,
                        old,
                        new: self.tile,
                    }),
                    _ => {}
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
        // `tile_at` found the cell, so both coordinates are non-negative.
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return 0;
        };
        let room = self.room();
        let (width, height) = (room.width, room.height);
        let mut seen = vec![false; room.tiles.len()];
        let mut cells = Vec::new();
        let mut stack = vec![(x, y)];
        while let Some((cx, cy)) = stack.pop() {
            let index = cy * width + cx;
            let (Some(done), Some(&tile)) = (seen.get_mut(index), room.tiles.get(index)) else {
                continue;
            };
            if *done || tile != target {
                continue;
            }
            *done = true;
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
}
