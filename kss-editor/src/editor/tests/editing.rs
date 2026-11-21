//! Painting, filling, objects, music, size and the undo history.

use kss_core::{CELL, Door, Object};

use super::editor;

#[test]
fn paint_is_one_undo_step_per_stroke() {
    let Some(mut e) = editor() else { return };
    e.tile = 0x0C;
    let before = e.room().tiles.clone();
    e.begin_group();
    for x in 0..5 {
        e.paint(x, 0);
    }
    e.end_group();
    assert!((0..5).all(|x| e.room().tile(x, 0) == 0x0C));
    assert!(e.is_dirty(1));
    assert!(e.undo());
    assert_eq!(e.room().tiles, before);
    assert!(!e.can_undo());
    assert!(e.redo());
    assert!((0..5).all(|x| e.room().tile(x, 0) == 0x0C));
}

#[test]
fn fill_rect_and_flood_fill_undo() {
    let Some(mut e) = editor() else { return };
    let before = e.room().tiles.clone();
    e.tile = 0x0C;
    assert!(e.fill_rect((2, 1), (0, 0)) > 0);
    for (x, y) in [(0, 0), (1, 0), (2, 0), (0, 1), (2, 1)] {
        assert_eq!(e.room().tile(x, y), 0x0C);
    }
    e.undo();
    assert_eq!(e.room().tiles, before);
    // Flood fill the sky region around (50, 0) with another tile.
    let sky = e.room().tile(50, 0);
    e.tile = if sky == 0x0D { 0x0E } else { 0x0D };
    let n = e.flood_fill(50, 0);
    assert!(n > 0);
    assert!(e.room().tiles.iter().filter(|&&t| t == e.tile).count() >= n);
    assert_eq!(e.room().tile(50, 0), e.tile);
    e.undo();
    assert_eq!(e.room().tiles, before);
    // Filling with the tile already there changes nothing.
    e.tile = e.room().tile(50, 0);
    assert_eq!(e.flood_fill(50, 0), 0);
    assert!(!e.can_undo());
}

#[test]
fn objects_add_move_remove_undo() {
    let Some(mut e) = editor() else { return };
    let before = e.room().objects.clone();
    e.new_object = (0x02, 3);
    let i = e.add_object(100, 50);
    assert_eq!(
        e.room().objects[i],
        Object {
            id: 2,
            variant: 3,
            x: 100,
            y: 50
        }
    );
    assert_eq!(e.selected_object, Some(i));
    assert_eq!(e.object_at(104, 47, 8), Some(i));
    e.begin_group();
    e.move_object(i, 110, 60);
    e.move_object(i, 120, 70);
    e.nudge_object(i, 1, 0);
    e.end_group();
    assert_eq!((e.room().objects[i].x, e.room().objects[i].y), (121, 70));
    // The whole drag is one step: one undo goes back to the placement.
    e.undo();
    assert_eq!((e.room().objects[i].x, e.room().objects[i].y), (100, 50));
    e.redo();
    assert!(e.remove_object(i));
    assert_eq!(e.selected_object, None);
    e.undo();
    e.undo();
    e.undo();
    assert_eq!(e.room().objects, before);
    // Positions clamp to the room.
    let i = e.add_object(-5, 100_000);
    assert_eq!(
        (e.room().objects[i].x, e.room().objects[i].y),
        (0, (8 * CELL - 1) as u16)
    );
}

#[test]
fn doors_music_and_resize_undo() {
    let Some(mut e) = editor() else { return };
    let doors = e.room().doors();
    if let Some(&door) = doors.first() {
        let moved = Door {
            dest_x: door.dest_x.wrapping_add(8),
            ..door
        };
        assert!(e.set_door(0, moved));
        assert_eq!(e.room().doors()[0], moved);
        e.undo();
        assert_eq!(e.room().doors()[0], door);
    }
    let music = e.room().music();
    assert!(e.set_music(music ^ 1));
    assert!(!e.set_music(music ^ 1));
    e.undo();
    assert_eq!(e.room().music(), music);
    let before = (e.room().width, e.room().height, e.room().tiles.clone());
    let rev = e.revision();
    assert!(e.resize(90, 9));
    assert_eq!((e.room().width, e.room().height), (90, 9));
    assert_eq!(e.room().tiles.len(), 90 * 9);
    assert!(e.revision() > rev);
    e.undo();
    assert_eq!(
        (e.room().width, e.room().height, e.room().tiles.clone()),
        before
    );
}

#[test]
fn switching_rooms_keeps_each_history() {
    let Some(mut e) = editor() else { return };
    e.tile = 0x0C;
    e.set_tile(0, 0, 0x0C);
    e.select_room(2).unwrap();
    assert!(!e.can_undo());
    e.select_room(1).unwrap();
    assert!(e.can_undo());
    assert_eq!(e.dirty_rooms(), vec![1]);
}

#[test]
fn undoing_a_delete_drops_the_selection() {
    let Some(mut e) = editor() else { return };
    assert!(e.remove_object(0));
    e.selected_object = Some(0);
    assert!(e.undo());
    assert_eq!(e.selected_object, None);
}
