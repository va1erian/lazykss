//! The sub-games' levels, against the original ROM.

use super::tests::rom;
use super::*;

#[test]
fn sub_games_cover_the_first_36_levels_in_order() {
    let mut next = 0;
    for game in SubGame::ALL {
        assert_eq!(game.levels().start, next, "{game:?}");
        next = game.levels().end;
        for level in game.levels() {
            assert_eq!(SubGame::of_level(level), Some(game));
        }
    }
    assert_eq!(next, FIRST_UNPLAYED_LEVEL);
    assert_eq!(SubGame::of_level(FIRST_UNPLAYED_LEVEL), None);
}

#[test]
fn every_room_of_a_sub_game_carries_its_header_byte() {
    let Some(rom) = rom() else { return };
    let tables = Tables::load(&rom);
    for level in &tables.levels {
        for index in level.rooms.clone() {
            let room = Room::load(&rom, &tables, index).unwrap();
            let byte = room.head()[0];
            match level.sub_game {
                Some(game) => assert_eq!(byte, game.header_byte(), "room {index}"),
                // The theatre, demo and dummy rooms use values from 7 up.
                None => assert!(byte >= 7, "room {index}: {byte}"),
            }
        }
    }
}

#[test]
fn sub_game_levels_have_their_rooms_and_names() {
    let Some(rom) = rom() else { return };
    let tables = Tables::load(&rom);
    let level = |index: usize| &tables.levels[index];
    assert_eq!(level(4).name, "Peanut Plains");
    assert_eq!(level(4).sub_game, Some(SubGame::DynaBlade));
    assert_eq!(level(8).rooms, 63..66);
    assert_eq!(level(11).sub_game, Some(SubGame::GourmetRace));
    assert_eq!(level(11).rooms, 68..69);
    assert_eq!(level(17).rooms.start, 154);
    assert_eq!(level(26).name, "Floria");
    assert_eq!(level(17).name, "Prologue");
    assert_eq!(level(26).rooms, 197..206);
    assert_eq!(level(35).name, "The Arena");
    assert_eq!(level(35).rooms, 262..284);
    assert_eq!(level(36).sub_game, None);
    assert_eq!(level(36).name, "Level 36");
}

#[test]
fn every_door_of_a_sub_game_leads_into_its_own_level() {
    let Some(rom) = rom() else { return };
    let tables = Tables::load(&rom);
    let mut doors = 0;
    for level in tables.levels.iter().filter(|l| l.sub_game.is_some()) {
        for index in level.rooms.clone() {
            for door in Room::load(&rom, &tables, index).unwrap().doors() {
                doors += 1;
                if let Destination::Room(room) = door.destination(level) {
                    assert!(level.rooms.contains(&room), "room {index}: {door:?}");
                }
            }
        }
    }
    assert!(doors > 400, "only {doors} doors");
}

#[test]
fn a_flagged_destination_names_a_room_and_keeps_its_flag() {
    let Some(rom) = rom() else { return };
    let tables = Tables::load(&rom);
    // Room 167 of Revenge of Meta Knight's level 19 (rooms 162-169) leads
    // to "4007": room 169. The game enters 4000 | n as n (emulator).
    let level = tables.level_of(167).unwrap();
    let door = Room::load(&rom, &tables, 167)
        .unwrap()
        .doors()
        .into_iter()
        .find(|d| d.dest_room == 0x4007)
        .unwrap();
    assert!(door.has_room_flag());
    assert_eq!(door.destination(level), Destination::Room(169));
    assert_eq!(door.retargeted(level, 165).unwrap().dest_room, 0x4003);
    let plain = Door {
        dest_room: 2,
        ..door
    };
    assert!(!plain.has_room_flag());
    assert_eq!(plain.retargeted(level, 165).unwrap().dest_room, 3);
    let exit = Door {
        dest_room: Door::LEVEL_EXIT,
        ..door
    };
    assert!(!exit.has_room_flag());
    assert_eq!(exit.destination(level), Destination::LevelExit);
    assert_eq!(
        Door {
            dest_room: 0x8001,
            ..door
        }
        .destination(level),
        Destination::Special(0x8001)
    );
}
