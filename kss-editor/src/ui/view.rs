//! What the map view shows and where: zoom, scroll, layers and overlays.
//! Pure state and coordinate maths, no drawing.

use kss_core::{CELL, Layers};

use super::scroll::Scroller;

pub const MIN_ZOOM: i32 = 1;
pub const MAX_ZOOM: i32 = 4;

#[derive(Debug, Clone)]
pub struct ViewState {
    pub zoom: i32,
    pub layers: Layers,
    pub grid: bool,
    pub doors: bool,
    pub objects: bool,
    pub scroller: Scroller,
    /// Room pixel under the pointer, for the hover highlight.
    pub hover: Option<(i32, i32)>,
    /// Cell corners of a rectangle being dragged.
    pub rect_drag: Option<((i32, i32), (i32, i32))>,
    pub focused: bool,
    /// Room size in cells, mirrored from the editor so scrolling can clamp.
    pub room_cells: (i32, i32),
    /// Where a middle-button pan started: pointer and scroll offset.
    pub pan: Option<((i32, i32), (i32, i32))>,
}

impl ViewState {
    pub fn new() -> Self {
        Self {
            zoom: 2,
            layers: Layers::default(),
            grid: false,
            doors: true,
            objects: true,
            scroller: Scroller::new(),
            hover: None,
            rect_drag: None,
            focused: false,
            room_cells: (0, 0),
            pan: None,
        }
    }

    /// Tells the view how big the room is; recomputes the scroll range.
    pub fn set_room(&mut self, width: usize, height: usize) {
        self.room_cells = (width as i32, height as i32);
        self.sync_content();
    }

    fn sync_content(&mut self) {
        let cell = CELL as i32 * self.zoom;
        self.scroller.content = (self.room_cells.0 * cell, self.room_cells.1 * cell);
        self.scroller.clamp();
    }

    /// The room pixel under a point of the node (may lie outside the room).
    pub fn to_room(&self, x: i32, y: i32) -> (i32, i32) {
        let (ox, oy) = self.scroller.offset;
        ((x + ox).div_euclid(self.zoom), (y + oy).div_euclid(self.zoom))
    }

    /// The cell containing a room pixel.
    pub fn cell_of(pos: (i32, i32)) -> (i32, i32) {
        (pos.0.div_euclid(CELL as i32), pos.1.div_euclid(CELL as i32))
    }

    /// The node point a room pixel is drawn at.
    pub fn to_screen(&self, pos: (i32, i32)) -> (i32, i32) {
        (pos.0 * self.zoom - self.scroller.offset.0, pos.1 * self.zoom - self.scroller.offset.1)
    }

    /// Sets the zoom, keeping the room pixel under `anchor` (a point of the
    /// node) where it is.
    pub fn set_zoom(&mut self, zoom: i32, anchor: (i32, i32)) -> bool {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        if zoom == self.zoom {
            return false;
        }
        let (rx, ry) = self.to_room(anchor.0, anchor.1);
        self.zoom = zoom;
        self.sync_content();
        self.scroller.offset = (rx * zoom - anchor.0, ry * zoom - anchor.1);
        self.scroller.clamp();
        true
    }

    /// Zooms about the middle of the visible area.
    pub fn set_zoom_centered(&mut self, zoom: i32) -> bool {
        let view = self.scroller.view();
        self.set_zoom(zoom, (view.width() / 2, view.height() / 2))
    }

    /// Scrolls so that the room pixel `pos` is in the middle of the view.
    pub fn center_on(&mut self, pos: (i32, i32)) {
        let view = self.scroller.view();
        self.scroller.offset = (pos.0 * self.zoom - view.width() / 2, pos.1 * self.zoom - view.height() / 2);
        self.scroller.clamp();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> ViewState {
        let mut v = ViewState::new();
        v.scroller.size = (800, 600);
        v.set_room(100, 8);
        v
    }

    #[test]
    fn coordinates_round_trip() {
        let mut v = view();
        v.scroller.offset = (480, 0);
        let pos = v.to_room(10, 20);
        assert_eq!(pos, ((10 + 480) / 2, 10));
        assert_eq!(v.to_screen(pos), (10, 20));
        assert_eq!(ViewState::cell_of((47, 23)), (1, 0));
        assert_eq!(ViewState::cell_of((-1, -25)), (-1, -2));
    }

    #[test]
    fn zoom_keeps_the_anchor_fixed_and_clamps() {
        let mut v = view();
        v.scroller.offset = (1000, 0);
        let before = v.to_room(300, 100);
        assert!(v.set_zoom(4, (300, 100)));
        let after = v.to_room(300, 100);
        assert!((before.0 - after.0).abs() <= 1 && (before.1 - after.1).abs() <= 1);
        assert!(!v.set_zoom(9, (0, 0)), "zoom is capped at 4x");
        assert_eq!(v.zoom, MAX_ZOOM);
        v.set_zoom(1, (0, 0));
        assert_eq!(v.scroller.content, (2400, 192));
    }
}
