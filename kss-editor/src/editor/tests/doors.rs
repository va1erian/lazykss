//! Doors, warp stars, treasure chests and their labels.

use kss_core::Door;
use kss_core::names::{DOOR_STAR, TREASURE_CHEST, WARP_STAR};

use super::{editor, temp};
use crate::editor::Editor;

#[test]
fn the_cave_lists_its_treasure_chests() {
    let Some(e) = editor() else { return };
    let chests = e.chests();
    // 58 chests: every treasure but 0 and 16, once each.
    assert_eq!(chests.len(), 58);
    let numbers: Vec<u8> = chests.iter().map(|c| c.treasure).collect();
    let expected: Vec<u8> = (0..60).filter(|n| *n != 0 && *n != 16).collect();
    assert_eq!(numbers, expected);
    let coin = chests.iter().find(|c| c.treasure == 1).unwrap();
    assert_eq!((coin.room, coin.x, coin.y), (80, 204, 124));
    assert_eq!(
        e.loaded(80).unwrap().room.objects[coin.object].id,
        TREASURE_CHEST
    );
}

#[test]
fn labels_resolve_doors_chests_and_warps() {
    let Some(mut e) = editor() else { return };
    e.select_room(7).unwrap();
    let door = e.room().doors()[0];
    assert_eq!(e.destination_label(&door), "room 8");
    e.select_room(80).unwrap();
    let chest = *e
        .room()
        .objects
        .iter()
        .find(|o| o.id == TREASURE_CHEST)
        .unwrap();
    assert_eq!(e.object_label(&chest), "Treasure 1: Gold Coin");
    e.select_room(134).unwrap();
    let warp = *e.room().objects.iter().find(|o| o.id == WARP_STAR).unwrap();
    assert_eq!(e.object_label(&warp), "Warp Star \u{2192} room 148");
}

#[test]
fn doors_and_warps_add_and_remove_in_one_step() {
    let Some(mut e) = editor() else { return };
    // Room 1: door 0 (with its door star) and door 1, used by the warp star.
    let (doors, objects) = (e.room().doors(), e.room().objects.clone());
    let i = e.add_door(100, 130).unwrap();
    assert_eq!(i, 2);
    assert_eq!(e.selected_door, Some(2));
    let door = e.room().doors()[2];
    assert_eq!((door.x, door.y, door.dest_room), (96, 120, 1));
    let star = *e.room().objects.last().unwrap();
    assert_eq!((star.id, star.x, star.y), (DOOR_STAR, 108, 132));
    assert!(e.undo());
    assert_eq!(e.room().doors(), doors);
    assert_eq!(e.room().objects, objects);
    assert!(e.redo());
    // A warp added after it uses door 3.
    let w = e.add_warp(300, 60).unwrap();
    assert_eq!(w, 3);
    assert!(!e.room().doors()[3].has_trigger());
    assert_eq!(e.warps_of(3).len(), 1);
    // Deleting door 0 takes its door star and renumbers both warp stars.
    assert!(e.remove_door(0));
    assert_eq!(e.room().doors().len(), 3);
    let warps = |e: &Editor| -> Vec<usize> {
        e.room()
            .objects
            .iter()
            .filter_map(|o| o.warp_door())
            .collect()
    };
    assert_eq!(warps(&e), vec![0, 2]);
    assert!(
        !e.room()
            .objects
            .iter()
            .any(|o| o.id == DOOR_STAR && (o.x, o.y) == (396, 132))
    );
    // Deleting the warp's door takes the warp star.
    assert!(e.remove_door(2));
    assert!(e.warps_of(2).is_empty());
    assert_eq!(warps(&e), vec![0]);
    // Each deletion is one undo step.
    assert!(e.undo());
    assert!(e.undo());
    assert_eq!(warps(&e), vec![1, 3]);
    assert!(e.undo());
    assert!(e.undo());
    assert_eq!(e.room().doors(), doors);
    assert_eq!(e.room().objects, objects);
    // FFFF would end the list.
    let first = e.room().doors()[0];
    assert!(!e.set_door(
        0,
        Door {
            dest_room: 0xFFFF,
            ..first
        }
    ));
}

#[test]
fn a_door_sharing_a_cell_keeps_one_star() {
    let Some(mut e) = editor() else { return };
    // Room 1's door 0 is on (384, 120); add a second door on that cell.
    let stars = |e: &Editor| {
        e.room()
            .objects
            .iter()
            .filter(|o| o.id == DOOR_STAR && (o.x, o.y) == (396, 132))
            .count()
    };
    let i = e.add_door(390, 125).unwrap();
    assert_eq!(stars(&e), 2);
    assert!(e.remove_door(i));
    assert_eq!(stars(&e), 1, "door 0 keeps its star");
    assert!(e.remove_door(0));
    assert_eq!(stars(&e), 0);
}

#[test]
fn an_added_door_survives_a_save() {
    let Some(mut e) = editor() else { return };
    e.select_room(75).unwrap();
    let blocks = e.room().blocks();
    let i = e.add_door(500, 300).unwrap();
    let door = Door {
        dest_room: 8,
        ..e.room().doors()[i]
    };
    assert!(e.set_door(i, door));
    let out = temp("door-save").join("out.sfc");
    e.save_as(&out).unwrap();
    let mut back = Editor::open(&out).unwrap();
    back.select_room(75).unwrap();
    assert_eq!(back.room().doors()[i], door);
    assert_eq!(back.destination_label(&door), "room 80");
    assert_eq!(back.room().blocks(), blocks);
    assert_eq!(back.chests().len(), 58);
    let _ = std::fs::remove_dir_all(temp("door-save"));
}
