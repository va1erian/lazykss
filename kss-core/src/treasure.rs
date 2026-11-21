//! The treasures of The Great Cave Offensive.
//!
//! A treasure is a chest object (`names::TREASURE_CHEST`) whose variant is
//! the treasure number. What each number is called and what it is worth is
//! fixed by the game, not by the room: the table below was read off the
//! screen by placing a chest of each number in Green Greens room 1 and
//! opening it (`tools/harness/experiment_treasures.py`). The values are
//! also in the ROM, at [`VALUE_TABLE`].

/// A treasure as the game names it when its chest is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Treasure {
    pub name: &'static str,
    /// In gold, as the game shows it.
    pub value: u32,
}

/// Treasures by number (the chest's variant).
const TREASURES: [(&str, u32); COUNT] = [
    ("Gold Medal", 10000),
    ("Gold Coin", 1000),
    ("Whip", 6800),
    ("Crystal Ball", 200000),
    ("Lucky Cat", 500),
    ("Seiryu Sword", 142000),
    ("Screw Ball", 80000),
    ("Echigo Candy", 8000),
    ("Zebra Mask", 278000),
    ("Star Stone", 82100),
    ("Beast's Fang", 7300),
    ("Bandanna", 1990),
    ("Springtime", 250000),
    ("Dime", 10),
    ("Glass Slippers", 120000),
    ("Goblet", 800),
    ("Saucepan", 10),
    ("Brass Knuckle", 20000),
    ("Amber Rose", 22100),
    ("Fish Fossil", 8250),
    ("Beast Fossil", 24220),
    ("Nunchuks", 55480),
    ("Bucket", 200),
    ("Summertime", 250000),
    ("100 dollar coin", 10000),
    ("Ancient Gem", 68000),
    ("Falcon Helmet", 41000),
    ("Dud", 30),
    ("Truth Mirror", 500000),
    ("Star Tiara", 408200),
    ("Turtle Shell", 800),
    ("Sword", 325000),
    ("Warrior Shield", 50000),
    ("Unicorn's Horn", 80300),
    ("Autumntime", 250000),
    ("Rice Bowl", 50),
    ("Tut's Mask", 160000),
    ("Mr. Saturn", 120000),
    ("Armor", 212000),
    ("Treasure Box", 100000),
    ("Mannequin", 3000),
    ("Gold Crown", 528000),
    ("King's Cape", 508000),
    ("Model Ship", 800000),
    ("Sun Ring", 800000),
    ("Wintertime", 250000),
    ("Katana", 990000),
    ("Charm", 8000),
    ("Xmas Tree", 40000),
    ("Kong's Barrel", 1500),
    ("Ramia's Scale", 12800),
    ("Shiny Bamboo", 600000),
    ("Tire", 1100),
    ("Spirit Charm", 78500),
    ("Pegasus Wing", 42800),
    ("Raccoon Doll", 8150),
    ("Shell Whistle", 82000),
    ("Orihalcon", 512000),
    ("Platinum Ring", 40000),
    ("Triforce", 800000),
];

/// Number of treasures. Chests of a higher number open on an empty window.
pub const COUNT: usize = 60;

/// File offset of the game's treasure value table: one 5-byte record per
/// treasure, a 24-bit value then a word holding a permutation of 0-59
/// (its use is not established).
pub const VALUE_TABLE: usize = 0xF_4B1A;

/// Treasure `number`, when it is one.
pub fn treasure(number: u8) -> Option<Treasure> {
    TREASURES
        .get(number as usize)
        .map(|&(name, value)| Treasure { name, value })
}

/// The sum of every treasure's value.
pub fn total_value() -> u64 {
    TREASURES.iter().map(|&(_, v)| v as u64).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_adds_up_to_the_games_total() {
        assert_eq!(total_value(), 9_999_990);
        assert_eq!(treasure(1).unwrap().name, "Gold Coin");
        assert_eq!(treasure(59).unwrap().value, 800_000);
        assert!(treasure(60).is_none());
    }

    #[test]
    fn values_match_the_rom() {
        let Some(path) = crate::Rom::locate() else {
            return;
        };
        let rom = crate::Rom::load(&path).unwrap();
        for number in 0..COUNT {
            let at = VALUE_TABLE + number * 5;
            let bytes = rom.slice(at, 3);
            let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
            assert_eq!(
                value,
                treasure(number as u8).unwrap().value,
                "treasure {number}"
            );
        }
    }
}
