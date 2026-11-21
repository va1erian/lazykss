//! The scratch buffer around the map and the stamp tool, driven with the
//! mouse and keyboard.

mod common;

use common::{at, check, key, mouse_down, mouse_move, mouse_up, shoot};
use kss_editor::editor::Tool;
use kss_editor::ui::{Layer, Msg};
use xui::Theme;
use xui::prelude::*;

#[test]
fn compose_in_the_buffer_and_stamp_it_on_the_map() {
    shoot("buffer", Theme::dark(), |s, editor, log| {
        s.emit(Msg::Layer(Layer::Grid));
        let (h, before) = {
            let e = editor.borrow();
            let e = e.as_ref().unwrap();
            (e.room().height as i32, e.room().tiles.clone())
        };
        let mid = |x: i32, y: i32| at(x * 24 + 12, y * 24 + 12);
        // Paint a two-tile motif in the buffer below the room.
        s.emit(Msg::SetTool(Tool::Paint));
        s.emit(Msg::PickTile(0x0C));
        mouse_down(s, mid(2, h + 1), MouseButton::Left);
        mouse_move(s, mid(3, h + 1));
        mouse_up(s, mid(3, h + 1), MouseButton::Left);
        s.emit(Msg::PickTile(0x0D));
        mouse_down(s, mid(2, h + 2), MouseButton::Left);
        mouse_up(s, mid(2, h + 2), MouseButton::Left);
        {
            let e = editor.borrow();
            let e = e.as_ref().unwrap();
            check(log, e.buffer().len() == 3, "three buffer cells painted");
            check(log, e.room().tiles == before, "the map is untouched");
            check(log, !e.is_dirty(1), "the buffer does not modify the room");
        }
        // Right-drag copies it with the stamp tool (S), a click pastes it.
        key(s, Key::S, Modifiers::NONE);
        mouse_down(s, mid(2, h + 1), MouseButton::Right);
        mouse_move(s, mid(3, h + 2));
        mouse_up(s, mid(3, h + 2), MouseButton::Right);
        check(
            log,
            editor
                .borrow()
                .as_ref()
                .unwrap()
                .stamp
                .as_ref()
                .is_some_and(|st| (st.width, st.height) == (2, 2)),
            "a 2x2 stamp was copied",
        );
        // A drag repeats it every two cells.
        mouse_down(s, mid(10, 1), MouseButton::Left);
        mouse_move(s, mid(13, 1));
        mouse_up(s, mid(13, 1), MouseButton::Left);
        {
            let e = editor.borrow();
            let e = e.as_ref().unwrap();
            for x in [10, 12] {
                check(log, e.room().tile(x, 1) == 0x0C, "stamp top row");
                check(log, e.room().tile(x, 2) == 0x0D, "stamp bottom row");
                check(log, e.room().tile(x + 1, 1) == 0x0C, "stamp top right");
            }
            check(log, e.is_dirty(1), "stamping the map modifies the room");
        }
        s.emit(Msg::Undo);
        check(
            log,
            editor.borrow().as_ref().unwrap().room().tiles == before,
            "one undo takes the whole stamp drag back",
        );
        // The Object tool does nothing in the buffer.
        s.emit(Msg::SetTool(Tool::AddObject));
        let objects = editor.borrow().as_ref().unwrap().room().objects.len();
        mouse_down(s, mid(4, h + 3), MouseButton::Left);
        mouse_up(s, mid(4, h + 3), MouseButton::Left);
        check(
            log,
            editor.borrow().as_ref().unwrap().room().objects.len() == objects,
            "no object is placed from the buffer",
        );
        s.emit(Msg::SetTool(Tool::Stamp));
        // Leave the ghost showing for the screenshot.
        mouse_move(s, mid(6, 3));
    });
}
