//! The side panel: its tabs, door editing and the treasure list.

mod common;

use common::{TAB_DOORS, TAB_OBJECTS, TAB_ROOM, TAB_TREASURES, check, key, shoot};
use kss_editor::editor::Tool;
use kss_editor::ui::{DoorField, Msg, SharedEditor};
use xui::Theme;
use xui::prelude::{Key, Modifiers};

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
fn f1_shows_the_guide() {
    for theme in [Theme::light(), Theme::dark()] {
        shoot("tab-help", theme, |s, _, _| {
            key(s, Key::F1, Modifiers::NONE);
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
        s.emit(Msg::DoorField(DoorField::DestX, 500.0));
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
        s.emit(Msg::DoorField(DoorField::DestRoom, 75.0));
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
