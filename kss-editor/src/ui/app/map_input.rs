//! The map and its toolbars: tools, zoom, layers, and the pointer and keys
//! on the map.

use kss_core::names::DOOR_STAR;
use xui::prelude::*;

use super::super::keys::{self, MapKey};
use super::super::map::MapEvent;
use super::{EditorApp, Gesture, Layer};
use crate::editor::{Tool, line_cells};

impl EditorApp {
    pub(super) fn set_tool(&mut self, tool: Tool) {
        self.ed_mut(|e| {
            e.end_group();
            e.tool = tool;
        });
        self.gesture = Gesture::Idle;
        self.h.map.get().update_view(|v| v.rect_drag = None);
        self.refresh_toolbar();
        self.say(tool.hint());
    }

    pub(super) fn zoom_by(&mut self, delta: i32) {
        self.h
            .map
            .get()
            .update_view(|v| v.set_zoom_centered(v.zoom.saturating_add(delta)));
        self.refresh_status();
    }

    pub(super) fn toggle_layer(&mut self, layer: Layer) {
        // The view owns the state; the strip is set to match, whether the
        // message came from a click on it or from elsewhere.
        let mut on = false;
        self.h.map.get().update_view(|v| {
            let flag = match layer {
                Layer::Background => &mut v.layers.background,
                Layer::Foreground => &mut v.layers.foreground,
                Layer::Grid => &mut v.grid,
                Layer::Doors => &mut v.doors,
                Layer::Objects => &mut v.objects,
            };
            *flag = !*flag;
            on = *flag;
        });
        let index = Layer::ALL.iter().position(|l| *l == layer).unwrap_or(0);
        self.h.layers.get().set_checked(index, on);
    }

    pub(super) fn map_event(&mut self, event: MapEvent) {
        match event {
            MapEvent::Hover { cell, pos } => {
                self.cursor = Some((cell, pos));
                self.refresh_status();
            }
            MapEvent::Leave => {
                self.cursor = None;
                self.refresh_status();
            }
            MapEvent::ViewChanged => self.refresh_status(),
            MapEvent::Press {
                button,
                cell,
                pos,
                mods,
            } => self.press(button, cell, pos, mods),
            MapEvent::Drag { cell, pos, .. } => {
                self.cursor = Some((cell, pos));
                self.drag(cell, pos);
            }
            MapEvent::Release { cell, pos, .. } => {
                self.cursor = Some((cell, pos));
                self.release(cell);
            }
            MapEvent::Key { key, mods } => {
                if let Some(action) = keys::map_key(key, mods) {
                    self.map_key(action);
                }
            }
        }
    }

    fn press(&mut self, button: MouseButton, cell: (i32, i32), pos: (i32, i32), mods: Modifiers) {
        // With no ROM the map shows an "Open ROM" button; a click on it opens the dialog.
        if self.ed(|_| ()).is_none() {
            if button == MouseButton::Left {
                self.request_open();
            }
            return;
        }
        let tool = self.ed(|e| e.tool).unwrap_or(Tool::Select);
        if tool == Tool::Stamp && button == MouseButton::Right {
            self.gesture = Gesture::Copy { start: cell };
            self.h
                .map
                .get()
                .update_view(|v| v.rect_drag = Some((cell, cell)));
            return;
        }
        let tile_tool = matches!(tool, Tool::Paint | Tool::Rect | Tool::Fill);
        // Right-click, or Alt+click with the paint tool, picks the tile.
        if (button == MouseButton::Right && tile_tool) || (mods.alt && tool == Tool::Paint) {
            if self
                .ed_mut(|e| e.pick_tile(cell.0, cell.1))
                .unwrap_or(false)
            {
                self.refresh_tile();
            }
            return;
        }
        if button != MouseButton::Left {
            return;
        }
        match tool {
            Tool::Select => self.select_at(pos),
            Tool::Paint => {
                let erase = mods.shift;
                self.ed_mut(|e| {
                    e.begin_group();
                    if erase {
                        e.erase(cell.0, cell.1);
                    } else {
                        e.paint(cell.0, cell.1);
                    }
                });
                self.gesture = Gesture::Paint { last: cell, erase };
                self.after_stroke();
            }
            Tool::Stamp => self.start_stamp(cell),
            Tool::Rect => {
                self.gesture = Gesture::Rect { start: cell };
                self.h
                    .map
                    .get()
                    .update_view(|v| v.rect_drag = Some((cell, cell)));
            }
            Tool::Fill => {
                let n = self.ed_mut(|e| e.flood_fill(cell.0, cell.1)).unwrap_or(0);
                self.after_stroke();
                if n > 0 {
                    self.say(&format!("Filled {n} cells"));
                }
            }
            Tool::AddObject => {
                self.ed_mut(|e| e.add_object(pos.0, pos.1));
                self.after_edit();
                self.select_object_row();
            }
        }
    }

    /// A Stamp click at `cell`: pastes the stamp there and starts a drag
    /// that repeats it.
    fn start_stamp(&mut self, cell: (i32, i32)) {
        if self.ed(|e| e.stamp.is_none()).unwrap_or(true) {
            self.say("Nothing to paste: right-drag over cells to copy them first");
            return;
        }
        self.ed_mut(|e| {
            e.begin_group();
            e.paste_stamp(cell);
        });
        self.gesture = Gesture::Stamp {
            origin: cell,
            last: cell,
        };
        self.after_stroke();
    }

    /// Where a stamp drag that began at `origin` pastes next with the
    /// pointer on `cell`: snapped to the stamp's own grid.
    fn stamp_step(&self, origin: (i32, i32), cell: (i32, i32)) -> Option<(i32, i32)> {
        let (w, h) = self
            .ed(|e| e.stamp.as_ref().map(|s| (s.width.max(1), s.height.max(1))))
            .flatten()?;
        let snap = |o: i32, c: i32, n: i32| {
            o.saturating_add(c.saturating_sub(o).div_euclid(n).saturating_mul(n))
        };
        Some((snap(origin.0, cell.0, w), snap(origin.1, cell.1, h)))
    }

    /// A Select click at room position `pos`: picks the object there (and
    /// starts moving it), else the door there.
    fn select_at(&mut self, pos: (i32, i32)) {
        // The hit radius grows as the map zooms out, so small sprites stay
        // clickable. The zoom is clamped to at least 1, `max` only guards
        // the division.
        let zoom = self.h.map.get().view().zoom.max(1);
        let radius = 8 + 4 / zoom;
        // A door's invisible star sits in the middle of its box: a click
        // there means the door (the star stays in the list).
        let hit = self
            .ed(|e| {
                e.object_at(pos.0, pos.1, radius).filter(|&i| {
                    e.room().objects.get(i).is_some_and(|o| {
                        !(o.id == DOOR_STAR && e.door_at(o.x.into(), o.y.into()).is_some())
                    })
                })
            })
            .flatten();
        let grabbed = hit.and_then(|index| {
            self.ed_mut(|e| {
                e.selected_object = Some(index);
                e.selected_door = None;
                e.begin_group();
                e.room().objects.get(index).copied()
            })
            .flatten()
            .map(|o| (index, o))
        });
        match grabbed {
            Some((index, o)) => {
                // Keep the grab offset, so the object does not jump to the pointer.
                self.gesture = Gesture::Move {
                    index,
                    dx: i32::from(o.x).saturating_sub(pos.0),
                    dy: i32::from(o.y).saturating_sub(pos.1),
                };
            }
            None if hit.is_none() => {
                self.ed_mut(|e| {
                    e.selected_object = None;
                    e.selected_door = e.door_at(pos.0, pos.1);
                });
            }
            None => {}
        }
        self.refresh_objects();
        self.refresh_doors();
        self.h.map.get().invalidate();
    }

    fn drag(&mut self, cell: (i32, i32), pos: (i32, i32)) {
        match self.gesture {
            Gesture::Paint { last, erase } => {
                if cell != last {
                    self.ed_mut(|e| {
                        for (x, y) in line_cells(last, cell) {
                            if erase {
                                e.erase(x, y);
                            } else {
                                e.paint(x, y);
                            }
                        }
                    });
                    self.gesture = Gesture::Paint { last: cell, erase };
                    self.after_stroke();
                }
            }
            Gesture::Stamp { origin, last } => {
                if let Some(at) = self.stamp_step(origin, cell)
                    && at != last
                {
                    self.ed_mut(|e| e.paste_stamp(at));
                    self.gesture = Gesture::Stamp { origin, last: at };
                    self.after_stroke();
                }
            }
            Gesture::Rect { start } | Gesture::Copy { start } => {
                self.h
                    .map
                    .get()
                    .update_view(|v| v.rect_drag = Some((start, cell)));
            }
            Gesture::Move { index, dx, dy } => {
                let (x, y) = (pos.0.saturating_add(dx), pos.1.saturating_add(dy));
                self.ed_mut(|e| e.move_object(index, x, y));
                self.after_stroke();
                self.refresh_object_fields();
            }
            Gesture::Idle => {}
        }
        self.refresh_status();
    }

    fn release(&mut self, cell: (i32, i32)) {
        match std::mem::replace(&mut self.gesture, Gesture::Idle) {
            Gesture::Paint { .. } | Gesture::Stamp { .. } => {
                self.ed_mut(|e| e.end_group());
                self.after_stroke();
            }
            Gesture::Copy { start } => {
                let size = self.ed_mut(|e| e.copy_stamp(start, cell)).flatten();
                self.h.map.get().update_view(|v| v.rect_drag = None);
                match size {
                    Some((w, h)) => {
                        self.say(&format!(
                            "Copied {w}x{h} cells: click to paste, drag to repeat"
                        ));
                    }
                    None => self.say("Nothing copied: those cells are empty"),
                }
            }
            Gesture::Rect { start } => {
                let n = self.ed_mut(|e| e.fill_rect(start, cell)).unwrap_or(0);
                self.h.map.get().update_view(|v| v.rect_drag = None);
                self.after_stroke();
                if n > 0 {
                    self.say(&format!("Filled {n} cells"));
                }
            }
            Gesture::Move { .. } => {
                self.ed_mut(|e| e.end_group());
                self.after_edit();
            }
            Gesture::Idle => {}
        }
    }

    fn map_key(&mut self, key: MapKey) {
        match key {
            MapKey::Tool(tool) => self.set_tool(tool),
            MapKey::Zoom(z) => {
                self.h.map.get().update_view(|v| v.set_zoom_centered(z));
                self.refresh_status();
            }
            MapKey::Escape => {
                self.gesture = Gesture::Idle;
                self.ed_mut(|e| {
                    e.end_group();
                    e.selected_object = None;
                    e.selected_door = None;
                });
                self.h.map.get().update_view(|v| v.rect_drag = None);
                self.refresh_objects();
            }
            MapKey::DeleteObject => {
                // The selected object, else the selected door.
                let removed = self
                    .ed_mut(|e| match (e.selected_object, e.selected_door) {
                        (Some(i), _) => e.remove_object(i),
                        (None, Some(i)) => e.remove_door(i),
                        (None, None) => false,
                    })
                    .unwrap_or(false);
                if removed {
                    self.after_edit();
                }
            }
            MapKey::Nudge(dx, dy) => {
                let moved = self
                    .ed_mut(|e| e.selected_object.is_some_and(|i| e.nudge_object(i, dx, dy)))
                    .unwrap_or(false);
                if moved {
                    self.after_stroke();
                    self.refresh_object_fields();
                    self.refresh_objects();
                }
            }
        }
    }
}
