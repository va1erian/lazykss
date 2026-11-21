//! The map view: a custom-painted, scrollable, zoomable node.
//!
//! The widget owns the view (scroll, zoom, overlays) and turns raw input into
//! [`MapEvent`]s in room coordinates. Editing decisions are the app's: it
//! receives the events as messages and calls the [`Editor`](crate::editor::Editor).

use std::cell::RefCell;
use std::rc::Rc;

use xui::backend::{Event, NodeKind, NodeSpec, Result};
use xui::geometry::{Rect, Size};
use xui::layout::Constraints;
use xui::prelude::*;
use xui::widget::{Control, Placeable};

use super::SharedEditor;
use super::map_paint;
use super::render::RoomImages;
use super::view::ViewState;

/// Pixels scrolled per wheel notch.
const WHEEL_STEP: i32 = 72;

/// Input on the map, with positions already converted to the room.
#[derive(Debug, Clone)]
pub enum MapEvent {
    Press {
        button: MouseButton,
        cell: (i32, i32),
        pos: (i32, i32),
        mods: Modifiers,
    },
    /// The pointer moved with a button held.
    Drag {
        cell: (i32, i32),
        pos: (i32, i32),
        mods: Modifiers,
    },
    Release {
        button: MouseButton,
        cell: (i32, i32),
        pos: (i32, i32),
    },
    Hover {
        cell: (i32, i32),
        pos: (i32, i32),
    },
    Leave,
    Key {
        key: Key,
        mods: Modifiers,
    },
    /// Zoom or scroll changed; the status bar should follow.
    ViewChanged,
}

struct Context<M: 'static> {
    ui: Ui<M>,
    id: xui::backend::WidgetId,
    state: Rc<RefCell<ViewState>>,
    wrap: Rc<dyn Fn(MapEvent) -> M>,
    /// The button of the gesture in progress, if any.
    pressed: std::cell::Cell<Option<MouseButton>>,
}

pub struct MapView<M: 'static> {
    control: Control<M>,
    state: Rc<RefCell<ViewState>>,
}

impl<M: 'static> MapView<M> {
    pub fn new(
        ui: &Ui<M>,
        editor: SharedEditor,
        wrap: impl Fn(MapEvent) -> M + 'static,
    ) -> Result<MapView<M>> {
        let control = Control::new(
            ui,
            &NodeSpec::new(NodeKind::Custom, Rect::default()).tab_stop(),
        )?;
        let state = Rc::new(RefCell::new(ViewState::new()));
        state.borrow_mut().scroller.dpi = ui.dpi();
        let cache = Rc::new(RefCell::new(RoomImages::new()));

        {
            let (state, theme) = (Rc::clone(&state), ui.theme_handle());
            control.set_painter(Rc::new(move |canvas| {
                map_paint::paint(
                    canvas,
                    &editor.borrow(),
                    &mut state.borrow_mut(),
                    &mut cache.borrow_mut(),
                    &theme.get(),
                );
            }));
        }
        let ctx = Context {
            ui: ui.clone(),
            id: control.id(),
            state: Rc::clone(&state),
            wrap: Rc::new(wrap),
            pressed: std::cell::Cell::new(None),
        };
        control.on_events(move |event| handle(&ctx, event));
        Ok(MapView { control, state })
    }

    /// Changes the view and repaints.
    pub fn update_view<R>(&self, f: impl FnOnce(&mut ViewState) -> R) -> R {
        let result = f(&mut self.state.borrow_mut());
        self.control.invalidate();
        result
    }

    /// A copy of the view, for reading zoom and overlays.
    pub fn view(&self) -> ViewState {
        self.state.borrow().clone()
    }

    pub fn invalidate(&self) {
        self.control.invalidate();
    }
}

impl<M: 'static> Placeable<M> for MapView<M> {
    fn id(&self) -> xui::backend::WidgetId {
        self.control.id()
    }

    /// Scrolls, so it takes whatever slot the layout gives it.
    fn measure(&self, _ui: &Ui<M>, _constraints: Constraints) -> Size {
        Size::new(0, 0)
    }
}

/// What handling an event asks of the node around it, done after the view's
/// borrow ends (focus and capture can dispatch events back into the node).
#[derive(Default)]
struct Out {
    focus: bool,
    capture: Option<bool>,
    event: Option<MapEvent>,
}

fn handle<M: 'static>(ctx: &Context<M>, event: &Event) -> Option<M> {
    let out = {
        let mut st = ctx.state.borrow_mut();
        process(ctx, &mut st, event)
    };
    let out = out?;
    if out.focus {
        ctx.ui.focus(ctx.id);
    }
    match out.capture {
        Some(true) => ctx.ui.set_capture(ctx.id),
        Some(false) => ctx.ui.release_capture(),
        None => {}
    }
    ctx.ui.invalidate(ctx.id);
    out.event.map(|e| (ctx.wrap)(e))
}

fn process<M: 'static>(ctx: &Context<M>, st: &mut ViewState, event: &Event) -> Option<Out> {
    let mut out = Out::default();
    match *event {
        Event::Resize { width, height } => {
            st.scroller.size = (width, height);
            st.scroller.clamp();
        }
        Event::SetFocus | Event::KillFocus => st.focused = matches!(event, Event::SetFocus),
        Event::MouseDown {
            x,
            y,
            button,
            modifiers,
        }
        | Event::MouseDoubleClick {
            x,
            y,
            button,
            modifiers,
        } => {
            out.focus = true;
            if st.scroller.press(x, y) {
                out.capture = st.scroller.dragging().then_some(true);
                return Some(out);
            }
            match button {
                MouseButton::Middle => {
                    st.pan = Some(((x, y), st.scroller.offset));
                    out.capture = Some(true);
                }
                MouseButton::Left | MouseButton::Right
                    if st.scroller.view().contains(xui::geometry::Point::new(x, y)) =>
                {
                    let pos = st.to_room(x, y);
                    ctx.pressed.set(Some(button));
                    out.capture = Some(true);
                    out.event = Some(MapEvent::Press {
                        button,
                        cell: ViewState::cell_of(pos),
                        pos,
                        mods: modifiers,
                    });
                }
                _ => return None,
            }
        }
        Event::MouseMove { x, y, modifiers } => {
            if st.scroller.drag_to(x, y) {
                return Some(out);
            }
            if let Some((start, offset)) = st.pan {
                st.scroller.offset = (offset.0 - (x - start.0), offset.1 - (y - start.1));
                st.scroller.clamp();
                out.event = Some(MapEvent::ViewChanged);
                return Some(out);
            }
            let pos = st.to_room(x, y);
            st.hover = Some(pos);
            let cell = ViewState::cell_of(pos);
            out.event = Some(if ctx.pressed.get().is_some() {
                MapEvent::Drag {
                    cell,
                    pos,
                    mods: modifiers,
                }
            } else {
                MapEvent::Hover { cell, pos }
            });
        }
        Event::MouseUp { x, y, button, .. } => {
            if st.scroller.end_drag() || (button == MouseButton::Middle && st.pan.take().is_some())
            {
                out.capture = Some(false);
            } else if ctx.pressed.get() == Some(button) {
                ctx.pressed.set(None);
                out.capture = Some(false);
                let pos = st.to_room(x, y);
                out.event = Some(MapEvent::Release {
                    button,
                    cell: ViewState::cell_of(pos),
                    pos,
                });
            } else {
                return None;
            }
        }
        Event::MouseLeave => {
            st.hover = None;
            if ctx.pressed.get().is_none() {
                out.event = Some(MapEvent::Leave);
            }
        }
        Event::CaptureChanged => {
            st.scroller.end_drag();
            st.pan = None;
            ctx.pressed.set(None);
        }
        Event::MouseWheel {
            delta,
            horizontal,
            x,
            y,
            modifiers,
        } => {
            let notches = f32::from(delta) / 120.0;
            if modifiers.ctrl && !horizontal {
                let zoom = st.zoom + if notches > 0.0 { 1 } else { -1 };
                if st.set_zoom(zoom, (x, y)) {
                    out.event = Some(MapEvent::ViewChanged);
                }
                return Some(out);
            }
            let amount = (notches * WHEEL_STEP as f32).round() as i32;
            // Rooms are wide and short: when the room fits the height the
            // wheel scrolls sideways, and Shift reaches the buffer above
            // and below.
            let sideways = horizontal
                || if st.room_fits_vertically() || st.scroller.max_offset().1 == 0 {
                    !modifiers.shift
                } else {
                    modifiers.shift
                };
            if sideways {
                // Wheel up (positive) moves left; a horizontal wheel's positive is right.
                let dx = if horizontal { amount } else { -amount };
                st.scroller.scroll_by(dx, 0);
            } else {
                st.scroller.scroll_by(0, -amount);
            }
            out.event = Some(MapEvent::ViewChanged);
        }
        Event::KeyDown {
            key,
            modifiers,
            system: false,
            ..
        } if !modifiers.ctrl => {
            out.event = Some(MapEvent::Key {
                key,
                mods: modifiers,
            });
        }
        _ => return None,
    }
    Some(out)
}
