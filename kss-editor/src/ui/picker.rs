//! The metatile picker: the room's tile sheet as a scrollable, clickable
//! custom-painted node.

use std::cell::RefCell;
use std::rc::Rc;

use kss_core::CELL;
use xui::backend::{Event, NodeKind, NodeSpec, Result};
use xui::geometry::{Point, Rect, Size};
use xui::image::Image;
use xui::layout::Constraints;
use xui::prelude::*;
use xui::widget::{Control, Placeable};

use super::SharedEditor;
use super::render::to_xui;
use super::scroll::Scroller;

/// Tiles per row of the sheet.
pub const COLUMNS: usize = 12;
const WHEEL_STEP: i32 = 72;

struct State {
    scroller: Scroller,
    /// The sheet and the room it was built for.
    sheet: Option<(usize, Image)>,
    pressed: bool,
}

pub struct TilePicker<M: 'static> {
    control: Control<M>,
    state: Rc<RefCell<State>>,
}

impl<M: 'static> TilePicker<M> {
    /// `wrap` maps a picked metatile id to the app's message.
    pub fn new(
        ui: &Ui<M>,
        editor: SharedEditor,
        wrap: impl Fn(u16) -> M + 'static,
    ) -> Result<TilePicker<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, Rect::default()))?;
        let mut scroller = Scroller::new();
        scroller.dpi = ui.dpi();
        let state = Rc::new(RefCell::new(State {
            scroller,
            sheet: None,
            pressed: false,
        }));

        {
            let (state, theme, editor) = (Rc::clone(&state), ui.theme_handle(), Rc::clone(&editor));
            control.set_painter(Rc::new(move |canvas| {
                paint(
                    canvas,
                    &editor.borrow(),
                    &mut state.borrow_mut(),
                    &theme.get(),
                );
            }));
        }
        {
            let (state, ui, id) = (Rc::clone(&state), ui.clone(), control.id());
            control.on_events(move |event| {
                let (message, capture) = {
                    let mut st = state.borrow_mut();
                    process(&mut st, event)
                };
                match capture {
                    Some(true) => ui.set_capture(id),
                    Some(false) => ui.release_capture(),
                    None => {}
                }
                if message.is_some()
                    || capture.is_some()
                    || matches!(event, Event::MouseWheel { .. } | Event::Resize { .. })
                {
                    ui.invalidate(id);
                }
                message.map(&wrap)
            });
        }
        Ok(TilePicker { control, state })
    }

    /// Scrolls so the cell of `tile` is visible.
    pub fn reveal(&self, tile: u16) {
        let mut st = self.state.borrow_mut();
        let top = (tile as usize / COLUMNS * CELL) as i32;
        let view = st.scroller.view().height();
        let offset = st.scroller.offset.1;
        if top < offset {
            st.scroller.offset.1 = top;
        } else if top + CELL as i32 > offset + view {
            st.scroller.offset.1 = top + CELL as i32 - view;
        }
        st.scroller.clamp();
        drop(st);
        self.control.invalidate();
    }

    pub fn invalidate(&self) {
        self.control.invalidate();
    }
}

impl<M: 'static> Placeable<M> for TilePicker<M> {
    fn id(&self) -> xui::backend::WidgetId {
        self.control.id()
    }

    fn measure(&self, _ui: &Ui<M>, _constraints: Constraints) -> Size {
        Size::new(0, 0)
    }
}

/// The tile id under a node point, if any.
fn tile_at(st: &State, count: usize, x: i32, y: i32) -> Option<u16> {
    let (ox, oy) = st.scroller.offset;
    let (cx, cy) = ((x + ox) / CELL as i32, (y + oy) / CELL as i32);
    if x < 0 || y < 0 || cx as usize >= COLUMNS {
        return None;
    }
    let id = cy as usize * COLUMNS + cx as usize;
    (id < count).then_some(id as u16)
}

fn process(st: &mut State, event: &Event) -> (Option<u16>, Option<bool>) {
    let count = st
        .sheet
        .as_ref()
        .map_or(0, |(_, img)| img.height() as usize / CELL * COLUMNS);
    match *event {
        Event::Resize { width, height } => {
            st.scroller.size = (width, height);
            st.scroller.clamp();
            (None, None)
        }
        Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } => {
            if st.scroller.press(x, y) {
                return (None, st.scroller.dragging().then_some(true));
            }
            if st.scroller.view().contains(Point::new(x, y)) {
                st.pressed = true;
                return (tile_at(st, count, x, y), Some(true));
            }
            (None, None)
        }
        Event::MouseMove { x, y, .. } => {
            if st.scroller.drag_to(x, y) {
                return (None, None);
            }
            if st.pressed {
                return (tile_at(st, count, x, y), None);
            }
            (None, None)
        }
        Event::MouseUp {
            button: MouseButton::Left,
            ..
        } => {
            if st.scroller.end_drag() || std::mem::take(&mut st.pressed) {
                return (None, Some(false));
            }
            (None, None)
        }
        Event::CaptureChanged => {
            st.scroller.end_drag();
            st.pressed = false;
            (None, None)
        }
        Event::MouseWheel {
            delta,
            horizontal: false,
            ..
        } => {
            st.scroller.scroll_by(
                0,
                -(f32::from(delta) / 120.0 * WHEEL_STEP as f32).round() as i32,
            );
            (None, None)
        }
        _ => (None, None),
    }
}

fn paint(
    canvas: &mut dyn xui::backend::Canvas,
    editor: &Option<super::Editor>,
    st: &mut State,
    theme: &Theme,
) {
    let origin = canvas.bounds();
    canvas.save();
    canvas.set_translation(origin.left as f32, origin.top as f32);
    let bounds = Rect::from_size(origin.size());
    st.scroller.size = (bounds.width(), bounds.height());
    st.scroller.dpi = canvas.dpi();
    canvas.fill_rect(bounds, theme.input_background);
    if let Some(editor) = editor {
        let room = editor.current_index();
        if st.sheet.as_ref().is_none_or(|(r, _)| *r != room) {
            let fill = [0x30, 0x30, 0x38, 0xFF];
            st.sheet =
                to_xui(&editor.tileset().metatile_sheet(COLUMNS, fill)).map(|img| (room, img));
        }
        if let Some((_, sheet)) = &st.sheet {
            st.scroller.content = (sheet.width() as i32, sheet.height() as i32);
            st.scroller.clamp();
            let view = st.scroller.view();
            let (ox, oy) = st.scroller.offset;
            canvas.push_clip(view);
            canvas.draw_image(
                sheet,
                Rect::new(
                    -ox,
                    -oy,
                    -ox + sheet.width() as i32,
                    -oy + sheet.height() as i32,
                ),
            );
            let id = editor.tile as usize;
            let (x, y) = (
                (id % COLUMNS * CELL) as i32 - ox,
                (id / COLUMNS * CELL) as i32 - oy,
            );
            let cell = Rect::new(x, y, x + CELL as i32, y + CELL as i32);
            canvas.stroke_rect(cell, theme.accent, 3.0);
            canvas.stroke_rect(
                Rect::new(cell.left + 2, cell.top + 2, cell.right - 2, cell.bottom - 2),
                xui::Color::rgb(255, 255, 255),
                1.0,
            );
            canvas.pop_clip();
        }
        st.scroller.paint(canvas, theme);
    }
    canvas.restore();
}
