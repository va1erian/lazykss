//! Scrollbars for custom-painted views.
//!
//! xui's own bars belong to its widgets; a custom node paints and handles its
//! own, using the geometry helpers in `xui::widget::scrollbar` so they look
//! and behave like the toolkit's.

use xui::backend::Canvas;
use xui::geometry::Rect;
use xui::theme::Theme;
use xui::widget::scrollbar::{self, Orientation, Scroll, THICKNESS, ThumbState, TrackHit};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Drag {
    Vertical { start: i32, offset: i32 },
    Horizontal { start: i32, offset: i32 },
}

/// Scroll position and bars of a view whose content is larger than itself.
/// All values are device pixels.
#[derive(Debug, Clone)]
pub struct Scroller {
    /// Size of the whole node.
    pub size: (i32, i32),
    /// Size of the content.
    pub content: (i32, i32),
    /// Scroll offset of the content's top-left corner.
    pub offset: (i32, i32),
    pub dpi: u32,
    drag: Option<Drag>,
}

impl Scroller {
    pub fn new() -> Self {
        Self { size: (0, 0), content: (0, 0), offset: (0, 0), dpi: 96, drag: None }
    }

    fn thickness(&self) -> i32 {
        THICKNESS.to_px(self.dpi).value().max(4)
    }

    /// Which bars show: `(vertical, horizontal)`.
    pub fn bars(&self) -> (bool, bool) {
        let t = self.thickness();
        let mut h = self.content.0 > self.size.0;
        let v = self.content.1 > self.size.1 - if h { t } else { 0 };
        h = self.content.0 > self.size.0 - if v { t } else { 0 };
        (v, h)
    }

    /// The area content is drawn into: the node minus its bars.
    pub fn view(&self) -> Rect {
        let (v, h) = self.bars();
        let t = self.thickness();
        Rect::new(0, 0, self.size.0 - if v { t } else { 0 }, self.size.1 - if h { t } else { 0 })
    }

    pub fn max_offset(&self) -> (i32, i32) {
        let view = self.view();
        ((self.content.0 - view.width()).max(0), (self.content.1 - view.height()).max(0))
    }

    pub fn clamp(&mut self) {
        let (mx, my) = self.max_offset();
        self.offset = (self.offset.0.clamp(0, mx), self.offset.1.clamp(0, my));
    }

    pub fn scroll_by(&mut self, dx: i32, dy: i32) {
        self.offset = (self.offset.0 + dx, self.offset.1 + dy);
        self.clamp();
    }

    fn v_track(&self) -> Option<Rect> {
        let (v, _) = self.bars();
        let view = self.view();
        v.then(|| Rect::new(view.right, 0, self.size.0, view.bottom))
    }

    fn h_track(&self) -> Option<Rect> {
        let (_, h) = self.bars();
        let view = self.view();
        h.then(|| Rect::new(0, view.bottom, view.right, self.size.1))
    }

    fn v_scroll(&self) -> Scroll {
        Scroll { viewport: self.view().height(), content: self.content.1, offset: self.offset.1 }
    }

    fn h_scroll(&self) -> Scroll {
        Scroll { viewport: self.view().width(), content: self.content.0, offset: self.offset.0 }
    }

    /// Whether a press at `(x, y)` lands on a bar; if so it is handled
    /// (page or start of a thumb drag) and the caller should capture the
    /// pointer while [`dragging`](Self::dragging).
    pub fn press(&mut self, x: i32, y: i32) -> bool {
        if let Some(track) = self.v_track().filter(|t| t.contains(xui::geometry::Point::new(x, y))) {
            let scroll = self.v_scroll();
            match scrollbar::hit(track, scroll, Orientation::Vertical, y, self.dpi) {
                Some(TrackHit::Thumb) => self.drag = Some(Drag::Vertical { start: y, offset: self.offset.1 }),
                Some(TrackHit::Before) => self.offset.1 = scrollbar::paged_offset(scroll, -1, scroll.viewport),
                Some(TrackHit::After) => self.offset.1 = scrollbar::paged_offset(scroll, 1, scroll.viewport),
                None => {}
            }
            return true;
        }
        if let Some(track) = self.h_track().filter(|t| t.contains(xui::geometry::Point::new(x, y))) {
            let scroll = self.h_scroll();
            match scrollbar::hit(track, scroll, Orientation::Horizontal, x, self.dpi) {
                Some(TrackHit::Thumb) => self.drag = Some(Drag::Horizontal { start: x, offset: self.offset.0 }),
                Some(TrackHit::Before) => self.offset.0 = scrollbar::paged_offset(scroll, -1, scroll.viewport),
                Some(TrackHit::After) => self.offset.0 = scrollbar::paged_offset(scroll, 1, scroll.viewport),
                None => {}
            }
            return true;
        }
        false
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Follows a thumb drag to `(x, y)`. Returns whether one is in progress.
    pub fn drag_to(&mut self, x: i32, y: i32) -> bool {
        match self.drag {
            Some(Drag::Vertical { start, offset }) => {
                if let Some(track) = self.v_track() {
                    self.offset.1 = scrollbar::offset_from_drag(
                        track,
                        self.v_scroll(),
                        Orientation::Vertical,
                        offset,
                        start,
                        y,
                        self.dpi,
                    );
                }
                true
            }
            Some(Drag::Horizontal { start, offset }) => {
                if let Some(track) = self.h_track() {
                    self.offset.0 = scrollbar::offset_from_drag(
                        track,
                        self.h_scroll(),
                        Orientation::Horizontal,
                        offset,
                        start,
                        x,
                        self.dpi,
                    );
                }
                true
            }
            None => false,
        }
    }

    pub fn end_drag(&mut self) -> bool {
        self.drag.take().is_some()
    }

    pub fn paint(&self, canvas: &mut dyn Canvas, theme: &Theme) {
        let state = |vertical: bool| match (self.drag, vertical) {
            (Some(Drag::Vertical { .. }), true) | (Some(Drag::Horizontal { .. }), false) => ThumbState::Pressed,
            _ => ThumbState::Normal,
        };
        if let Some(track) = self.v_track() {
            scrollbar::paint_state(canvas, track, self.v_scroll(), Orientation::Vertical, *theme, state(true));
        }
        if let Some(track) = self.h_track() {
            scrollbar::paint_state(canvas, track, self.h_scroll(), Orientation::Horizontal, *theme, state(false));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_appear_only_on_overflow_and_offsets_clamp() {
        let mut s = Scroller::new();
        s.size = (400, 300);
        s.content = (300, 200);
        assert_eq!(s.bars(), (false, false));
        s.content = (2400, 200);
        assert_eq!(s.bars(), (false, true));
        s.content = (2400, 295);
        // The horizontal bar takes room from the view, so the vertical one is needed too.
        assert_eq!(s.bars(), (true, true));
        s.scroll_by(100_000, 100_000);
        let max = s.max_offset();
        assert_eq!(s.offset, max);
        assert!(max.0 > 0 && max.1 > 0);
        s.scroll_by(-100_000, -100_000);
        assert_eq!(s.offset, (0, 0));
    }

    #[test]
    fn thumb_drag_moves_the_offset() {
        let mut s = Scroller::new();
        s.size = (400, 300);
        s.content = (400, 3000);
        let (v, _) = s.bars();
        assert!(v);
        let t = s.v_track().unwrap();
        assert!(s.press(t.left + 3, t.top + 5));
        assert!(s.dragging());
        s.drag_to(t.left + 3, t.top + 105);
        assert!(s.offset.1 > 0);
        assert!(s.end_drag());
    }
}
