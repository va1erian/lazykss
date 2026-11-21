//! Drives the real window headlessly: injects mouse and keyboard input into
//! the offscreen backend, checks the editor model and writes screenshots to
//! `target-shots/`. Every test passes silently when no ROM can be found.

use std::cell::RefCell;
use std::rc::Rc;

use kss_core::Rom;
use kss_editor::editor::Tool;
use kss_editor::ui::{Msg, SharedEditor, WINDOW, build_app, new_shared};
use xui::backend::Event;
use xui::prelude::*;
use xui::xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui::{Dip, Theme};

/// Where the map view sits in the 1380x860 window (see the layout report)
/// and its scroll/zoom at start.
const MAP: (i32, i32) = (262, 78);
const ZOOM: i32 = 2;
/// Tab header positions of the side panel.
const TAB_OBJECTS: (i32, i32) = (1106, 88);
const TAB_DOORS: (i32, i32) = (1167, 88);
const TAB_ROOM: (i32, i32) = (1225, 88);

type Log = Rc<RefCell<Vec<String>>>;

fn check(log: &Log, ok: bool, what: &str) {
    if !ok {
        log.borrow_mut().push(what.to_string());
    }
}

/// Room pixel to window pixel, with the view unscrolled.
fn at(px: i32, py: i32) -> (i32, i32) {
    (MAP.0 + px * ZOOM, MAP.1 + py * ZOOM)
}

fn mouse_down(s: &Stage<'_, Msg>, (x, y): (i32, i32), button: MouseButton) {
    s.inject(Event::MouseDown {
        x,
        y,
        button,
        modifiers: Modifiers::NONE,
    });
}

fn mouse_move(s: &Stage<'_, Msg>, (x, y): (i32, i32)) {
    s.inject(Event::MouseMove {
        x,
        y,
        modifiers: Modifiers::NONE,
    });
}

fn mouse_up(s: &Stage<'_, Msg>, (x, y): (i32, i32), button: MouseButton) {
    s.inject(Event::MouseUp {
        x,
        y,
        button,
        modifiers: Modifiers::NONE,
    });
}

fn key(s: &Stage<'_, Msg>, key: Key, modifiers: Modifiers) {
    s.inject(Event::KeyDown {
        key,
        modifiers,
        repeat: 1,
        system: false,
    });
}

/// Builds the app, runs `step`, saves the capture as `<name>-light.png`
/// (or `-dark`) and fails if `step` logged a failed check. Returns `false`
/// without doing anything when there is no ROM.
fn shoot(
    name: &str,
    theme: Theme,
    step: impl FnOnce(&Stage<'_, Msg>, &SharedEditor, &Log) + 'static,
) -> bool {
    if Rom::locate().is_none() {
        return false;
    }
    shoot_rom(name, theme, None, step)
}

/// Like [`shoot`], opening `rom` instead of the located ROM.
fn shoot_rom(
    name: &str,
    theme: Theme,
    rom: Option<std::path::PathBuf>,
    step: impl FnOnce(&Stage<'_, Msg>, &SharedEditor, &Log) + 'static,
) -> bool {
    let shared = new_shared();
    let log: Log = Rc::default();
    let (build_shared, step_shared, step_log) =
        (Rc::clone(&shared), Rc::clone(&shared), Rc::clone(&log));
    let variant = if theme.is_dark { "dark" } else { "light" };
    let image = render_with(
        Snapshot::new(Dip(WINDOW.0 as f32), Dip(WINDOW.1 as f32)).theme(theme),
        move |ui| build_app(ui, rom, build_shared),
        move |stage| step(stage, &step_shared, &step_log),
    )
    .expect("the window renders");
    let dir = std::path::Path::new("target-shots");
    std::fs::create_dir_all(dir).unwrap();
    image
        .save_png(dir.join(format!("{name}-{variant}.png")))
        .unwrap();
    let failures = log.borrow();
    assert!(failures.is_empty(), "failed checks: {failures:?}");
    true
}

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
        key(
            s,
            Key::Z,
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        check(
            log,
            editor.borrow().as_ref().unwrap().room().tiles == before,
            "Ctrl+Z reverts the stroke",
        );
        key(
            s,
            Key::Y,
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
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
        s.emit(Msg::Layer(kss_editor::ui::Layer::Grid));
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
fn compose_in_the_buffer_and_stamp_it_on_the_map() {
    shoot("buffer", Theme::dark(), |s, editor, log| {
        s.emit(Msg::Layer(kss_editor::ui::Layer::Grid));
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
        // Leave the ghost showing for the screenshot.
        mouse_move(s, mid(6, 3));
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
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        });
        s.inject(Event::MouseWheel {
            delta: 120,
            horizontal: false,
            x: p.0,
            y: p.1,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
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

#[test]
fn side_panel_tabs() {
    for (name, tab) in [
        ("tab-objects", TAB_OBJECTS),
        ("tab-doors", TAB_DOORS),
        ("tab-room", TAB_ROOM),
    ] {
        shoot(name, Theme::light(), move |s, editor, log| {
            s.click(tab.0, tab.1);
            let doors = editor.borrow().as_ref().unwrap().room().doors().len();
            if tab == TAB_DOORS && doors > 0 {
                s.emit(Msg::DoorSelect(0));
            } else if tab == TAB_OBJECTS {
                s.emit(Msg::ObjectSelect(1));
                check(
                    log,
                    editor.borrow().as_ref().unwrap().selected_object == Some(1),
                    "list selection reaches the editor",
                );
            }
        });
    }
}

#[test]
fn door_edit_and_destination_jump() {
    shoot("door-goto", Theme::light(), |s, editor, log| {
        s.click(TAB_DOORS.0, TAB_DOORS.1);
        let doors = editor.borrow().as_ref().unwrap().room().doors();
        check(log, !doors.is_empty(), "room 1 has doors");
        s.emit(Msg::DoorSelect(0));
        s.emit(Msg::DoorField(kss_editor::ui::DoorField::DestX, 500.0));
        check(
            log,
            editor.borrow().as_ref().unwrap().room().doors()[0].dest_x == 500,
            "the door field edits the door",
        );
        s.emit(Msg::DoorGoto);
        // Destinations count from the level's first room (0 for Green Greens).
        let dest = match editor.borrow().as_ref().unwrap().tables().level_of(1) {
            Some(level) => level.rooms.start + doors[0].dest_room as usize,
            None => usize::MAX,
        };
        check(
            log,
            editor.borrow().as_ref().unwrap().current_index() == dest,
            "Go to destination opens the target room",
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

const TAB_TREASURES: (i32, i32) = (1293, 88);

#[test]
fn treasure_list_opens_the_room_of_a_chest() {
    shoot("treasures", Theme::light(), |s, editor, log| {
        s.click(TAB_TREASURES.0, TAB_TREASURES.1);
        // Row 1 is treasure 1, the Gold Coin, in room 80.
        s.emit(Msg::TreasureSelect(1));
        {
            let e = editor.borrow();
            let e = e.as_ref().unwrap();
            check(log, e.current_index() == 80, "the chest's room is open");
            let selected = e.selected_object.map(|i| e.room().objects[i]);
            check(
                log,
                selected.is_some_and(|o| o.id == 0xB8 && o.variant == 1),
                "the chest is selected",
            );
        }
        // Placing a missing treasure arms the Object tool with its chest.
        s.emit(Msg::TreasureSelect(16));
        s.emit(Msg::TreasurePlace);
        let e = editor.borrow();
        let e = e.as_ref().unwrap();
        check(log, e.tool == Tool::AddObject, "the Object tool is on");
        check(log, e.new_object == (0xB8, 16), "with treasure 16's chest");
    });
}

#[test]
fn cave_doors_count_from_the_level_and_can_be_added() {
    shoot("cave-doors", Theme::dark(), |s, editor, log| {
        // Room 74 (the cave's third room): door 0 is stored as 8, room 80.
        s.emit(Msg::TreasureSelect(2));
        s.click(TAB_DOORS.0, TAB_DOORS.1);
        s.emit(Msg::DoorSelect(0));
        let door = |e: &SharedEditor| e.borrow().as_ref().unwrap().room().doors()[0];
        check(
            log,
            editor.borrow().as_ref().unwrap().current_index() == 74,
            "room 74 is open",
        );
        check(log, door(editor).dest_room == 8, "door 0 is stored as 8");
        // Setting "To room" stores the level-relative value.
        s.emit(Msg::DoorField(kss_editor::ui::DoorField::DestRoom, 75.0));
        check(log, door(editor).dest_room == 3, "room 75 is stored as 3");
        let before = editor.borrow().as_ref().unwrap().room().doors().len();
        s.emit(Msg::DoorAdd);
        s.emit(Msg::WarpAdd);
        {
            let e = editor.borrow();
            let e = e.as_ref().unwrap();
            check(
                log,
                e.room().doors().len() == before + 2,
                "a door and a warp were added",
            );
            check(
                log,
                e.warps_of(before + 1).len() == 1,
                "the warp star uses the new door record",
            );
        }
        s.emit(Msg::DoorSelect(0));
        s.emit(Msg::DoorGoto);
        check(
            log,
            editor.borrow().as_ref().unwrap().current_index() == 75,
            "Go to destination follows the level-relative value",
        );
        // Show a selected door in the capture.
        s.emit(Msg::DoorSelect(1));
    });
}
