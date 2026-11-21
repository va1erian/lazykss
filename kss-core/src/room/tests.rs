//! Loading, round trips and saving, against the original ROM.

use super::*;
use crate::rom::Rom;

pub(super) fn rom() -> Option<Rom> {
    Rom::locate().map(|path| Rom::load(&path).expect("ROM loads"))
}

#[test]
fn green_greens_room_1() {
    let Some(rom) = rom() else { return };
    let tables = Tables::load(&rom);
    assert_eq!(tables.room_count(), 319);
    assert_eq!(tables.levels[0].rooms, 0..6);
    assert_eq!(tables.levels[3].rooms, 24..27);
    let room = Room::load(&rom, &tables, 1).unwrap();
    assert_eq!(room.offset, 0x30_20EF);
    assert_eq!((room.width, room.height), (100, 8));
    assert_eq!(room.music(), 0x0F);
    assert_eq!(room.tile(15, 0), 0x0A0);
    assert_eq!(room.tile(0, 7), 0x00C);
    assert_eq!(room.objects.len(), 8);
    assert_eq!(
        room.objects[0],
        Object {
            variant: 0,
            id: 0x72,
            x: 0x18C,
            y: 0x84
        }
    );
    assert_eq!(
        room.objects[2],
        Object {
            variant: 0,
            id: 0x1A,
            x: 0x22D,
            y: 0x74
        }
    );
    let doors = room.doors();
    assert_eq!(doors.len(), 2);
    assert_eq!(
        doors[0],
        Door {
            dest_room: 2,
            x: 0x180,
            y: 0x78,
            dest_x: 0x9C,
            dest_y: 0x6C
        }
    );
    assert!(!doors[1].has_trigger());
    assert_eq!(room.stored_len, 0x30_225F - 0x30_20EF);
}

#[test]
fn every_room_round_trips() {
    let Some(rom) = rom() else { return };
    let tables = Tables::load(&rom);
    let mut loaded = 0;
    for index in 0..tables.room_count() {
        let Ok(room) = Room::load(&rom, &tables, index) else {
            continue;
        };
        loaded += 1;
        let block = room.encode().unwrap();
        // Re-parse the encoded block from a scratch image.
        let mut copy = rom.clone();
        let at = 0x2F_E9A0;
        copy.write(at, &block);
        copy.write_long_pointer(tables.room_table + index * 4, at);
        let moved = Room::load(&copy, &Tables::load(&copy), index).unwrap();
        assert_eq!(moved.tiles, room.tiles, "room {index}");
        assert_eq!(moved.objects, room.objects, "room {index}");
        assert_eq!(moved.head(), room.head(), "room {index}");
    }
    assert!(loaded >= 300, "only {loaded} rooms loaded");
}

#[test]
fn saving_unchanged_rooms_keeps_them_in_place() {
    let Some(original) = rom() else { return };
    let mut rom = original.clone();
    let mut tables = Tables::load(&rom);
    for index in tables.levels[0].rooms.start..tables.levels[3].rooms.end {
        let mut room = Room::load(&rom, &tables, index).unwrap();
        let report = room.save(&mut rom, &mut tables).unwrap();
        assert!(
            !report.moved,
            "room {index}: {} > {}",
            report.len, report.previous_len
        );
    }
}

#[test]
fn a_grown_room_moves_and_reloads() {
    let Some(original) = rom() else { return };
    let mut rom = original.clone();
    let mut tables = Tables::load(&rom);
    let mut room = Room::load(&rom, &tables, 1).unwrap();
    // Noise does not compress: the block must outgrow its slot.
    let mut seed = 7u32;
    for tile in room.tiles.iter_mut() {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        *tile = (seed >> 16) as u16 % 300;
    }
    room.objects.push(Object {
        variant: 0,
        id: 1,
        x: 100,
        y: 100,
    });
    let report = room.save(&mut rom, &mut tables).unwrap();
    assert!(report.moved);
    let back = Room::load(&rom, &Tables::load(&rom), 1).unwrap();
    assert_eq!(back.tiles, room.tiles);
    assert_eq!(back.objects, room.objects);
    // The neighbours are untouched.
    for index in [0, 2, 17] {
        assert_eq!(
            Room::load(&rom, &tables, index).unwrap(),
            Room::load(&original, &Tables::load(&original), index).unwrap()
        );
    }
}

#[test]
fn door_destinations_count_from_the_level() {
    let Some(rom) = rom() else { return };
    let tables = Tables::load(&rom);
    assert_eq!(tables.levels[GREAT_CAVE_OFFENSIVE].rooms, 72..153);
    assert_eq!(
        tables.levels[GREAT_CAVE_OFFENSIVE].name,
        "The Great Cave Offensive"
    );
    // Float Islands room 7 leads to its level's room "2": room 8.
    let level = tables.level_of(7).unwrap();
    let door = Room::load(&rom, &tables, 7).unwrap().doors()[0];
    assert_eq!(door.dest_room, 2);
    assert_eq!(door.destination(level), Destination::Room(8));
    assert_eq!(Door::relative_to(level, 8), Some(2));
    assert_eq!(Door::relative_to(level, 5), None);
    // The cave's first room leads to room 74, which leads back.
    let cave = tables.level_of(72).unwrap();
    let first = Room::load(&rom, &tables, 72).unwrap().doors()[0];
    assert_eq!(first.destination(cave), Destination::Room(74));
    let back = Room::load(&rom, &tables, 74).unwrap().doors();
    assert!(
        back.iter()
            .any(|d| d.destination(cave) == Destination::Room(80))
    );
    // Read as levels, doors pair up; read as rooms of the table, they do not.
    let mut paired = [0usize; 2];
    let loaded: Vec<Option<Room>> = (0..tables.room_count())
        .map(|i| Room::load(&rom, &tables, i).ok())
        .collect();
    let target = |relative: bool, from: usize, d: &Door| -> Option<usize> {
        let level = tables.level_of(from)?;
        match (relative, d.destination(level)) {
            (true, Destination::Room(r)) => Some(r),
            (false, _) if d.dest_room < 0x4000 => Some(d.dest_room as usize),
            _ => None,
        }
    };
    for (relative, count) in [true, false].into_iter().zip(paired.iter_mut()) {
        for (from, room) in loaded.iter().enumerate() {
            let Some(room) = room else { continue };
            for door in room.doors().iter().filter(|d| d.has_trigger()) {
                let Some(Some(to)) = target(relative, from, door).map(|t| loaded.get(t)) else {
                    continue;
                };
                let Some(to) = to else { continue };
                let to_index = target(relative, from, door).unwrap();
                if to
                    .doors()
                    .iter()
                    .any(|b| target(relative, to_index, b) == Some(from))
                {
                    *count += 1;
                }
            }
        }
    }
    assert!(paired[0] > 200 && paired[1] < 10, "{paired:?}");
}

#[test]
fn warp_stars_name_their_door() {
    let Some(rom) = rom() else { return };
    let tables = Tables::load(&rom);
    // Room 1: the warp star at the end uses door 1, which has no trigger.
    let room = Room::load(&rom, &tables, 1).unwrap();
    let warp = room
        .objects
        .iter()
        .find(|o| o.warp_door().is_some())
        .unwrap();
    assert_eq!(warp.warp_door(), Some(1));
    assert!(!room.doors()[1].has_trigger());
    // Room 134 (the cave): variant 8, the ninth door, also without one.
    let room = Room::load(&rom, &tables, 134).unwrap();
    let warp = room
        .objects
        .iter()
        .find(|o| o.warp_door().is_some())
        .unwrap();
    assert_eq!(warp.warp_door(), Some(8));
    assert!(!room.doors()[8].has_trigger());
}
