//! Opening and saving: the start state, Save and the Open ROM dialog.

mod common;

use common::{at, check, mouse_down, mouse_up, shoot, shoot_rom};
use kss_core::Rom;
use kss_editor::editor::Tool;
use kss_editor::ui::Msg;
use xui::Theme;
use xui::prelude::*;

#[test]
fn opens_room_one_with_a_cached_image() {
    shoot("start", Theme::light(), |_, editor, log| {
        let e = editor.borrow();
        let e = e.as_ref();
        check(
            log,
            e.is_some_and(|e| e.current_index() == 1),
            "room 1 is open",
        );
        check(
            log,
            e.is_some_and(|e| (e.room().width, e.room().height) == (100, 8)),
            "100x8",
        );
    });
}

#[test]
fn save_goes_to_an_edited_copy_and_never_over_the_source() {
    let Some(original) = Rom::locate() else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("kss-editor-ui-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("Kirby Super Star (USA).sfc");
    std::fs::copy(&original, &source).unwrap();
    let before = std::fs::read(&source).unwrap();
    let expect_dir = dir.clone();
    shoot_rom(
        "saved",
        Theme::light(),
        Some(source.clone()),
        move |s, editor, log| {
            s.emit(Msg::SetTool(Tool::Paint));
            s.emit(Msg::PickTile(0x0C));
            mouse_down(s, at(3 * 24 + 12, 2 * 24 + 12), MouseButton::Left);
            mouse_up(s, at(3 * 24 + 12, 2 * 24 + 12), MouseButton::Left);
            check(
                log,
                editor.borrow().as_ref().unwrap().is_dirty(1),
                "an edit marks the room",
            );
            s.emit(Msg::Save);
            let e = editor.borrow();
            let e = e.as_ref().unwrap();
            check(log, !e.any_dirty(), "saving clears the modified marks");
            let edited = expect_dir.join("Kirby Super Star (USA) (edited).sfc");
            check(
                log,
                e.save_target() == edited,
                "Save writes next to the source, to '(edited)'",
            );
            check(log, edited.is_file(), "the edited file exists");
        },
    );
    assert_eq!(
        std::fs::read(&source).unwrap(),
        before,
        "the source ROM was not touched"
    );
    let edited = dir.join("Kirby Super Star (USA) (edited).sfc");
    let reloaded = kss_editor::editor::Editor::open(&edited).unwrap();
    assert_eq!(reloaded.room().tile(3, 2), 0x0C);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn open_dialog_and_the_empty_state() {
    // The in-window file dialog (the offscreen backend has no native one).
    shoot("open-dialog", Theme::light(), |s, _, _| {
        s.emit(Msg::OpenRom)
    });
    // No ROM at all: the map shows its Open ROM button.
    let missing = std::path::PathBuf::from("does-not-exist.sfc");
    shoot_rom(
        "empty",
        Theme::light(),
        Some(missing.clone()),
        |_, editor, log| {
            check(log, editor.borrow().is_none(), "no editor without a ROM");
        },
    );
    // Clicking the empty map asks for a file, which opens the dialog.
    shoot_rom("empty-click", Theme::light(), Some(missing), |s, _, _| {
        mouse_down(s, (600, 300), MouseButton::Left);
        mouse_up(s, (600, 300), MouseButton::Left);
    });
}
