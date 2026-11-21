//! Editing: blocks, doors and resizing, against the original ROM.

use super::tests::rom;
use super::*;
use crate::rom::Rom;

#[test]
fn breakable_blocks_round_trip() {
    let Some(original) = rom() else { return };
    let mut rom = original.clone();
    let mut tables = Tables::load(&rom);
    // Room 3: four star blocks in row 4, their caps in row 3.
    let stars = Room::load(&rom, &tables, 3).unwrap();
    let blocks = stars.blocks();
    assert_eq!(blocks.len(), 8);
    assert_eq!(
        blocks[0],
        Block {
            x: 19,
            y: 3,
            after: 0,
            extra: 0
        }
    );
    assert_eq!(
        blocks[4],
        Block {
            x: 19,
            y: 4,
            after: 0xAC,
            extra: 0x57
        }
    );
    // Room 1 has none; add one and reload it from the image.
    let mut room = Room::load(&rom, &tables, 1).unwrap();
    assert!(room.blocks().is_empty());
    let doors = room.doors();
    room.set_tile(3, 5, 3);
    room.set_blocks(&[Block {
        x: 3,
        y: 5,
        after: 0,
        extra: 0,
    }]);
    room.save(&mut rom, &mut tables).unwrap();
    let back = Room::load(&rom, &tables, 1).unwrap();
    assert_eq!(
        back.blocks(),
        [Block {
            x: 3,
            y: 5,
            after: 0,
            extra: 0
        }]
    );
    assert_eq!(back.doors(), doors);
    assert_eq!(back.tiles, room.tiles);
    assert_eq!((back.width, back.height), (100, 8));
    // Syncing a room that already lists its blocks changes nothing.
    let tiles = crate::gfx::Tileset::for_room(&original, &Tables::load(&original), &stars);
    let mut synced = stars.clone();
    let was = |room: &Room| {
        let (width, tiles) = (room.width, room.tiles.clone());
        move |x: usize, y: usize| tiles.get(y * width + x).copied()
    };
    assert!(!synced.sync_blocks(|t| tiles.is_breakable(t), was(&stars), &blocks));
    // A new star block gets a record; painting it away drops it again.
    synced.set_tile(2, 2, 0x56);
    assert!(synced.sync_blocks(|t| tiles.is_breakable(t), was(&stars), &blocks));
    assert!(synced.blocks().contains(&Block {
        x: 2,
        y: 2,
        after: 0,
        extra: 0
    }));
    let with_block = synced.clone();
    synced.set_tile(2, 2, 0);
    assert!(synced.sync_blocks(|t| tiles.is_breakable(t), was(&with_block), &blocks));
    assert_eq!(synced.blocks(), blocks);
    // A listed block painted away and put back keeps what it breaks into.
    synced.set_tile(19, 4, 0);
    assert!(synced.sync_blocks(|t| tiles.is_breakable(t), was(&stars), &blocks));
    assert_eq!(synced.blocks().len(), 7);
    let without = synced.clone();
    synced.set_tile(19, 4, 0x56);
    assert!(synced.sync_blocks(|t| tiles.is_breakable(t), was(&without), &blocks));
    assert_eq!(synced.blocks(), blocks);
    // Resizing keeps each record on its cell.
    let mut wide = stars.clone();
    wide.resize(40, 8);
    assert_eq!(wide.blocks(), blocks);
    wide.resize(20, 8);
    assert_eq!(wide.blocks(), [blocks[0], blocks[4]]);
    // Writing a room's own list back changes nothing.
    let mut same = stars.clone();
    same.set_blocks(&blocks);
    assert_eq!(same, stars);
}

#[test]
fn doors_can_be_added_and_removed() {
    let Some(original) = rom() else { return };
    let mut rom = original.clone();
    let mut tables = Tables::load(&rom);
    let mut room = Room::load(&rom, &tables, 3).unwrap();
    let (doors, blocks, objects) = (room.doors(), room.blocks(), room.objects.clone());
    let new = Door {
        dest_room: 1,
        x: 48,
        y: 96,
        dest_x: 60,
        dest_y: 108,
    };
    assert!(room.insert_door(doors.len(), new));
    assert!(!room.insert_door(99, new));
    assert!(!room.insert_door(
        0,
        Door {
            dest_room: 0xFFFF,
            ..new
        }
    ));
    assert_eq!(room.doors().len(), doors.len() + 1);
    // The sections after the door list moved with it.
    assert_eq!(room.blocks(), blocks);
    room.save(&mut rom, &mut tables).unwrap();
    let back = Room::load(&rom, &tables, 3).unwrap();
    assert_eq!(back.doors()[doors.len()], new);
    assert_eq!(back.blocks(), blocks);
    assert_eq!(back.objects, objects);
    // Removing it gives the original room back, byte for byte.
    let mut again = back.clone();
    assert_eq!(again.remove_door(doors.len()), Some(new));
    assert_eq!(
        again.head(),
        Room::load(&original, &Tables::load(&original), 3)
            .unwrap()
            .head()
    );
    // A room can lose every door.
    again.set_doors(&[]);
    assert!(again.doors().is_empty());
    assert_eq!(again.blocks(), blocks);
}

#[test]
fn resize_keeps_the_top_left() {
    let Some(rom) = rom() else { return };
    let tables = Tables::load(&rom);
    let mut room = Room::load(&rom, &tables, 1).unwrap();
    let corner = room.tile(15, 0);
    room.resize(50, 10);
    assert_eq!(room.tiles.len(), 500);
    assert_eq!(room.tile(15, 0), corner);
    assert_eq!(room.tile(49, 9), 0);
}

/// A room whose head is too short for its own offsets: every accessor
/// answers without panicking, and encoding reports an error.
#[test]
fn a_corrupt_head_does_not_panic() {
    let mut room = Room {
        index: 0,
        offset: 0,
        stored_len: 0,
        head: vec![0; 10],
        size_at: 100,
        doors_end: 200,
        width: 3,
        height: 2,
        tiles: vec![1; 4],
        objects: Vec::new(),
        tail: Vec::new(),
    };
    assert_eq!(room.music(), 0);
    assert_eq!(room.gfx_header(), [0; 24]);
    assert_eq!(room.tile(5, 5), 0);
    room.set_tile(usize::MAX, usize::MAX, 7);
    room.patch_head(usize::MAX, &[1, 2, 3]);
    assert!(room.section(9).is_empty());
    assert!(room.blocks().is_empty());
    assert!(room.doors().is_empty());
    let door = Door {
        dest_room: 1,
        x: 0,
        y: 0,
        dest_x: 0,
        dest_y: 0,
    };
    assert!(!room.set_door(0, door));
    room.set_doors(&[door]);
    room.set_blocks(&[Block {
        x: 1,
        y: 1,
        after: 0,
        extra: 0,
    }]);
    assert_eq!(room.head(), &[0; 10]);
    assert!(room.encode().is_err());
    room.resize(usize::MAX, 2);
    assert_eq!((room.width, room.height), (3, 2));
    room.resize(2, 2);
    assert!(room.encode().is_err());
}

/// An image whose tables and rooms are noise loads what it can and
/// reports the rest.
#[test]
fn a_noisy_image_does_not_panic() {
    let mut seed = 1u32;
    let mut data: Vec<u8> = (0..crate::rom::ROM_SIZE)
        .map(|_| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (seed >> 16) as u8
        })
        .collect();
    data[0x7FC0..0x7FC0 + 18].copy_from_slice(b"KIRBY SUPER DELUXE");
    let rom = Rom::new(data).unwrap();
    let tables = Tables::load(&rom);
    for index in 0..tables.room_count().min(2000) {
        if let Ok(room) = Room::load(&rom, &tables, index) {
            let _ = room.encode();
            let _ = (room.blocks(), room.doors());
        }
    }
    for index in 0..tables.gfx_count.min(2000) {
        let _ = tables.resource(&rom, index);
    }
    let _ = free_space(&rom);
}
