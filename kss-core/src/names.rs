//! Human-readable names for object and song ids (community research:
//! Super Famicom Development Wiki, Proto K and darklink).

/// The invisible marker placed on each door (variant 16 on a goal door).
pub const DOOR_STAR: u8 = 0x72;
/// A warp star: its variant is the index of its door record (§3 of the
/// documentation).
pub const WARP_STAR: u8 = 0x75;
/// A Great Cave Offensive treasure chest: its variant is the treasure
/// number (see `treasure`).
pub const TREASURE_CHEST: u8 = 0xB8;

/// A Gourmet Race sign: variant 0 starts the race, 16 is a checkpoint (the
/// end of courses 1 and 2), 32 the goal. A course without one never starts
/// or never ends (emulator, `experiment_gourmet.py`).
pub const RACE_SIGN: u8 = 0xC0;
/// A Revenge of Meta Knight dialogue: its variant is the line, 0-24, shown
/// when Kirby comes near (emulator, `experiment_romk.py`).
pub const DIALOGUE: u8 = 0xBC;

/// What a race sign's variant makes it.
pub fn race_sign(variant: u8) -> &'static str {
    match variant {
        0 => "start",
        16 => "checkpoint",
        32 => "goal",
        _ => "unknown variant",
    }
}

/// Name of an object id, when known.
pub fn object_name(id: u8) -> Option<&'static str> {
    Some(match id {
        0x01 => "Waddle Dee",
        0x02 => "Waddle Doo",
        0x03 => "Sir Kibble",
        0x06 => "Cappy",
        0x0A => "Bronto Burt",
        0x0D => "Gordo",
        0x12 => "Knuckle Joe",
        0x18 => "Scarfy",
        0x1A => "Twizzy",
        0x20 => "Blade Knight",
        0x26 => "Birdon",
        0x28 => "Rocky",
        0x29 => "Chilly",
        0x2C => "T.A.C.",
        0x42 => "Karate Man",
        0x43 => "Poppy Bros. Sr.",
        0x44 => "Chef Kawasaki",
        0x45 => "Frosty",
        0x46 => "Bonkers",
        0x47 => "Bugzzy",
        0x50 => "Whispy Woods",
        0x51 => "Lololo & Lalala",
        0x52 => "Kracko Jr.",
        0x53 => "Kracko",
        0x54 => "King Dedede",
        0x55 => "Dyna Blade",
        0x5A => "Meta Knight",
        0x60 => "Nova's heart",
        0x65 => "King Dedede (race start)",
        DOOR_STAR => "Door star",
        0x73 => "Stage clear",
        WARP_STAR => "Warp Star",
        0x77 => "Vertical autoscroll",
        0x78 => "Mine cart",
        0x7A => "Launch cannon",
        0x80 => "1-Up",
        0x81 => "Invincibility candy",
        0x82 => "Maxim Tomato",
        0x83..=0x8D => "Food",
        0x90..=0x95 => "Ability item",
        0xA0..=0xA6 => "Stage intro",
        TREASURE_CHEST => "Treasure chest",
        DIALOGUE => "Dialogue",
        RACE_SIGN => "Race sign",
        0xC1 => "Arrow sign",
        0xC2..=0xD1 => "Race food",
        0xD2 => "Maxim Tomato (race)",
        _ => return None,
    })
}

/// Bit 7 of a room's music byte: the room starts no song and stops the one
/// playing (emulator, `tools/harness/experiment_music_bit.py`). Revenge of
/// Meta Knight's rooms with dialogue use it; a script starts the music.
pub const SILENT_ROOM: u8 = 0x80;

/// What a room's music byte plays, in words.
pub fn music_label(byte: u8) -> String {
    let song = byte & !SILENT_ROOM;
    let name = song_name(song).unwrap_or("unnamed song");
    if byte & SILENT_ROOM == 0 {
        name.to_string()
    } else {
        format!("silent (bit 7 set; song {song:02X}, {name})")
    }
}

/// Name of a song id, when known. Songs from 0x02 on in this list are named
/// after the stage whose rooms play them (from the room data).
pub fn song_name(id: u8) -> Option<&'static str> {
    Some(match id {
        0x01 => "Stage intro",
        0x02 => "Peanut Plains",
        0x05 => "Boss",
        0x07 => "King Dedede",
        0x08 => "Candy Mountain",
        0x0B => "Cocoa Cave",
        0x0D => "Mallow Castle",
        0x0F => "Green Greens",
        0x10 => "Float Islands",
        0x11 => "Bubbly Clouds",
        0x17 => "Guide",
        0x23 | 0x24 => "Gourmet Race",
        0x28 | 0x29 | 0x2A | 0x2C => "The Great Cave Offensive",
        0x2D => "Marx",
        0x31 => "Arena",
        0x32 | 0x33 => "Revenge of Meta Knight",
        0x37 => "Dyna Blade",
        _ => return None,
    })
}
