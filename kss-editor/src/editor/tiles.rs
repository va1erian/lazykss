//! Painting and filling the tile map of the current room and the scratch
//! buffer around it.

use std::collections::HashSet;

use super::{Cell, Editor};

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
    pub(super) fn cell_index(&self, x: i32, y: i32) -> Option<usize> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        let r = self.room();
        (x < r.width && y < r.height).then(|| y * r.width + x)
    }

    pub fn tile_at(&self, x: i32, y: i32) -> Option<u16> {
        let index = self.cell_index(x, y)?;
        self.room().tiles.get(index).copied()
    }

    /// Sets one cell of the map or the buffer to `id`. Returns whether it
    /// changed.
    pub fn set_tile(&mut self, x: i32, y: i32, id: u16) -> bool {
        self.set_cells([((x, y), Some(id))]) > 0
    }

    /// Paints the current tile at a cell.
    pub fn paint(&mut self, x: i32, y: i32) -> bool {
        self.set_tile(x, y, self.tile)
    }

    /// Picks the tile under a cell of the map or the buffer as the current
    /// one.
    pub fn pick_tile(&mut self, x: i32, y: i32) -> bool {
        match self.cell(x, y).tile() {
            Some(id) => {
                self.tile = id;
                true
            }
            None => false,
        }
    }

    /// Fills the cells between two corners (inclusive, any order) with the
    /// current tile, on the map and in the buffer. Returns how many cells
    /// changed.
    pub fn fill_rect(&mut self, a: (i32, i32), b: (i32, i32)) -> usize {
        let (l, t, r, bottom) = self.canvas_cells();
        let (x0, x1) = (a.0.min(b.0).max(l), a.0.max(b.0).min(r - 1));
        let (y0, y1) = (a.1.min(b.1).max(t), a.1.max(b.1).min(bottom - 1));
        let tile = Some(self.tile);
        let cells: Vec<_> = (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| ((x, y), tile)))
            .collect();
        self.set_cells(cells)
    }

    /// Replaces the 4-connected region of equal cells around a cell with the
    /// current tile. A region of the map stays on the map, one of the buffer
    /// in the buffer. Returns how many cells changed.
    pub fn flood_fill(&mut self, x: i32, y: i32) -> usize {
        let target = self.cell(x, y);
        if target == Cell::Outside || target.tile() == Some(self.tile) {
            return 0;
        }
        let tile = Some(self.tile);
        let mut seen = HashSet::new();
        let mut region = Vec::new();
        let mut stack = vec![(x, y)];
        while let Some((cx, cy)) = stack.pop() {
            if !seen.insert((cx, cy)) || self.cell(cx, cy) != target {
                continue;
            }
            region.push(((cx, cy), tile));
            // The region ends at the buffer's edge, so these never saturate.
            stack.extend([
                (cx.saturating_sub(1), cy),
                (cx.saturating_add(1), cy),
                (cx, cy.saturating_sub(1)),
                (cx, cy.saturating_add(1)),
            ]);
        }
        self.set_cells(region)
    }
}
