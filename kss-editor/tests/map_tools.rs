//! The map tools driven with the mouse and keyboard: paint, select and
//! move, rectangle, flood fill, add object, zoom and pan.

mod common;

use common::{CTRL, TAB_OBJECTS, at, check, key, mouse_down, mouse_move, mouse_up, shoot};
use kss_editor::editor::Tool;
use kss_editor::ui::{Layer, Msg};
use xui::Theme;
use xui::backend::Event;
use xui::prelude::*;

#[test]
fn paint_stroke_with_the_mouse_is_one_undo_step() {
    shoot("paint", Theme::dark(), |s, editor, log| {
        s.emit(Msg::SetTool(Tool::Paint));
        s.emit(Msg::PickTile(0x0C));
        let before = editor.borrow().as_ref().unwrap().room().tiles.clone();
        // Cells (3..=8, 2): drag from the middle of one to the middle of another.
        mouse_down(s, at(3 * 24 + 12, 2 * 24 + 12), MouseButton::Left);
        mouse_move(s, at(5 * 24 + 12, 2 * 24 + 12));
        mouse_move(s, at(8 * 24 + 12, 2 * 24 + 12));
        mouse_up(s, at(8 * 24 + 12, 2 * 24 + 12), MouseButton::Left);
        {
            let e = editor.borrow();
            let e = e.as_ref().unwrap();
            check(
                log,
                (3..=8).all(|x| e.room().tile(x, 2) == 0x0C),
                "the stroke painted 3..=8",
            );
            check(log, e.is_dirty(1), "room 1 is marked modified");
        }
        // Right-click picks the tile under the pointer.
        mouse_down(s, at(10 * 24 + 12, 7 * 24 + 12), MouseButton::Right);
        mouse_up(s, at(10 * 24 + 12, 7 * 24 + 12), MouseButton::Right);
        let picked = editor.borrow().as_ref().unwrap().tile;
        check(
            log,
            picked == editor.borrow().as_ref().unwrap().room().tile(10, 7),
            "right-click picked a tile",
        );
        // Ctrl+Z through the keyboard, as the window-wide shortcut.
        key(s, Key::Z, CTRL);
        check(
            log,
            editor.borrow().as_ref().unwrap().room().tiles == before,
            "Ctrl+Z reverts the stroke",
        );
        key(s, Key::Y, CTRL);
        check(
            log,
            editor.borrow().as_ref().unwrap().room().tile(8, 2) == 0x0C,
            "Ctrl+Y redoes it",
        );
        s.emit(Msg::Undo);
        check(
            log,
            editor.borrow().as_ref().unwrap().room().tiles == before,
            "one undo reverts the stroke",
        );
        s.emit(Msg::Redo);
        check(
            log,
            editor.borrow().as_ref().unwrap().room().tile(8, 2) == 0x0C,
            "redo repaints it",
        );
    });
}

#[test]
fn select_drag_nudge_and_delete_objects() {
    shoot("objects", Theme::light(), |s, editor, log| {
        let (index, o) = {
            let e = editor.borrow();
            let e = e.as_ref().unwrap();
            // The first object that is on screen at the start.
            let i = e
                .room()
                .objects
                .iter()
                .position(|o| (o.x as i32) < 340 && (o.y as i32) < 180)
                .expect("an object near the start");
            (i, e.room().objects[i])
        };
        let (x, y) = (o.x as i32, o.y as i32);
        mouse_down(s, at(x, y), MouseButton::Left);
        check(
            log,
            editor.borrow().as_ref().unwrap().selected_object == Some(index),
            "click selects the object",
        );
        mouse_move(s, at(x + 10, y + 5));
        mouse_move(s, at(x + 20, y + 10));
        mouse_up(s, at(x + 20, y + 10), MouseButton::Left);
        let moved = editor.borrow().as_ref().unwrap().room().objects[index];
        check(
            log,
            (moved.x as i32, moved.y as i32) == (x + 20, y + 10),
            "dragging moved it by the pointer delta",
        );
        key(s, Key::RIGHT, Modifiers::NONE);
        key(
            s,
            Key::DOWN,
            Modifiers {
                shift: true,
                ..Modifiers::NONE
            },
        );
        let nudged = editor.borrow().as_ref().unwrap().room().objects[index];
        check(
            log,
            (nudged.x as i32, nudged.y as i32) == (x + 21, y + 18),
            "arrow keys nudge by 1, shift by 8",
        );
        s.emit(Msg::Undo);
        s.emit(Msg::Undo);
        s.emit(Msg::Undo);
        check(
            log,
            editor.borrow().as_ref().unwrap().room().objects[index] == o,
            "three undos restore it",
        );
        s.emit(Msg::Redo);
        s.emit(Msg::Redo);
        s.emit(Msg::Redo);
        key(s, Key::DELETE, Modifiers::NONE);
        let count = editor.borrow().as_ref().unwrap().room().objects.len();
        check(log, count == 7, "Delete removed the selected object");
        s.emit(Msg::Undo);
        check(
            log,
            editor.borrow().as_ref().unwrap().room().objects.len() == 8,
            "undo brings it back",
        );
        // Show the selection in the screenshot.
        mouse_down(s, at(x + 21, y + 18), MouseButton::Left);
        mouse_up(s, at(x + 21, y + 18), MouseButton::Left);
        s.click(TAB_OBJECTS.0, TAB_OBJECTS.1);
    });
}

#[test]
fn rectangle_flood_fill_and_add_object() {
    shoot("tools", Theme::light(), |s, editor, log| {
        s.emit(Msg::Layer(Layer::Grid));
        s.emit(Msg::PickTile(0x0C));
        s.emit(Msg::SetTool(Tool::Rect));
        mouse_down(s, at(2 * 24 + 5, 24 + 5), MouseButton::Left);
        mouse_move(s, at(4 * 24 + 5, 2 * 24 + 5));
        mouse_up(s, at(4 * 24 + 5, 2 * 24 + 5), MouseButton::Left);
        {
            let e = editor.borrow();
            let e = e.as_ref().unwrap();
            check(
                log,
                (2..=4).all(|x| (1..=2).all(|y| e.room().tile(x, y) == 0x0C)),
                "the rectangle is filled",
            );
            check(
                log,
                e.room().tile(5, 1) != 0x0C,
                "outside the rectangle is untouched",
            );
        }
        s.emit(Msg::PickTile(0x0D));
        s.emit(Msg::SetTool(Tool::Fill));
        mouse_down(s, at(3 * 24 + 5, 24 + 5), MouseButton::Left);
        mouse_up(s, at(3 * 24 + 5, 24 + 5), MouseButton::Left);
        check(
            log,
            editor.borrow().as_ref().unwrap().room().tile(2, 2) == 0x0D,
            "flood fill replaced the 0x0C region",
        );
        s.emit(Msg::SetTool(Tool::AddObject));
        let before = editor.borrow().as_ref().unwrap().room().objects.len();
        mouse_down(s, at(200, 100), MouseButton::Left);
        mouse_up(s, at(200, 100), MouseButton::Left);
        let e = editor.borrow();
        let e = e.as_ref().unwrap();
        check(
            log,
            e.room().objects.len() == before + 1,
            "the Object tool placed an object",
        );
        check(log, e.selected_object == Some(before), "and selected it");
        check(
            log,
            e.room().objects[before].x == 200 && e.room().objects[before].y == 100,
            "at the clicked pixel",
        );
    });
}

#[test]
fn zoom_scroll_and_pan_the_view() {
    shoot("zoom", Theme::light(), |s, _editor, log| {
        // Ctrl+wheel zooms in about the pointer; the status bar follows.
        let p = at(300, 100);
        s.inject(Event::MouseWheel {
            delta: 120,
            horizontal: false,
            x: p.0,
            y: p.1,
            modifiers: CTRL,
        });
        s.inject(Event::MouseWheel {
            delta: 120,
            horizontal: false,
            x: p.0,
            y: p.1,
            modifiers: CTRL,
        });
        // The wheel alone scrolls sideways in a room that is not tall enough to scroll.
        s.inject(Event::MouseWheel {
            delta: -120,
            horizontal: false,
            x: p.0,
            y: p.1,
            modifiers: Modifiers::NONE,
        });
        // Middle-drag pans.
        mouse_down(s, (600, 300), MouseButton::Middle);
        mouse_move(s, (500, 300));
        mouse_up(s, (500, 300), MouseButton::Middle);
        s.emit(Msg::ZoomOut);
        check(log, true, "no panic");
    });
}
