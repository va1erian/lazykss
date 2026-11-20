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
        _ => return None,
    })
}

/// Name of a song id, when known.
pub fn song_name(id: u8) -> Option<&'static str> {
    Some(match id {
        0x01 => "Stage intro",
        0x05 => "Boss",
        0x07 => "King Dedede",
        0x0F => "Green Greens",
        0x10 => "Float Islands",
        0x11 => "Bubbly Clouds",
        0x2D => "Marx",
        0x31 => "Arena",
        _ => return None,
    })
}
