//! The sub-games of Kirby Super Star, and which levels of the level table
//! belong to each.
//!
//! The level table does not say which sub-game a level is part of; the
//! ranges here were read off the rooms and checked in the emulator. Byte 0
//! of every room block of a sub-game holds the same value
//! ([`SubGame::header_byte`], checked by the tests), but the game does not
//! act on it: changing it showed no difference in play. The sub-game a room
//! plays in comes from the menu and the level table.

use std::ops::Range;

/// A game of the main menu that is played through rooms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubGame {
    SpringBreeze,
    DynaBlade,
    GourmetRace,
    GreatCaveOffensive,
    RevengeOfMetaKnight,
    MilkyWayWishes,
    TheArena,
}

/// Levels 36 and up: the opening theatre, demo and dummy rooms.
pub const FIRST_UNPLAYED_LEVEL: usize = 36;

impl SubGame {
    /// Every sub-game, in menu order.
    pub const ALL: [SubGame; 7] = [
        SubGame::SpringBreeze,
        SubGame::DynaBlade,
        SubGame::GourmetRace,
        SubGame::GreatCaveOffensive,
        SubGame::RevengeOfMetaKnight,
        SubGame::MilkyWayWishes,
        SubGame::TheArena,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SubGame::SpringBreeze => "Spring Breeze",
            SubGame::DynaBlade => "Dyna Blade",
            SubGame::GourmetRace => "Gourmet Race",
            SubGame::GreatCaveOffensive => "The Great Cave Offensive",
            SubGame::RevengeOfMetaKnight => "Revenge of Meta Knight",
            SubGame::MilkyWayWishes => "Milky Way Wishes",
            SubGame::TheArena => "The Arena",
        }
    }

    /// The indices in the level table of this sub-game's levels.
    pub fn levels(self) -> Range<usize> {
        match self {
            SubGame::SpringBreeze => 0..4,
            SubGame::DynaBlade => 4..11,
            SubGame::GourmetRace => 11..15,
            SubGame::GreatCaveOffensive => 15..17,
            SubGame::RevengeOfMetaKnight => 17..26,
            SubGame::MilkyWayWishes => 26..35,
            SubGame::TheArena => 35..FIRST_UNPLAYED_LEVEL,
        }
    }

    /// Byte 0 of every room block of the sub-game.
    pub fn header_byte(self) -> u8 {
        match self {
            SubGame::SpringBreeze => 0,
            SubGame::DynaBlade => 1,
            SubGame::GourmetRace => 2,
            SubGame::GreatCaveOffensive => 3,
            SubGame::RevengeOfMetaKnight => 4,
            SubGame::MilkyWayWishes => 5,
            SubGame::TheArena => 6,
        }
    }

    /// The sub-game level `level` of the level table belongs to.
    pub fn of_level(level: usize) -> Option<SubGame> {
        SubGame::ALL
            .into_iter()
            .find(|g| g.levels().contains(&level))
    }
}

/// The name of level `index` of the level table, when it is known.
pub(super) fn level_name(index: usize) -> Option<&'static str> {
    LEVEL_NAMES.get(index).copied()
}

/// Names of levels 0-35, in level-table order. How each was identified is
/// in the documentation (§2a); the Revenge of Meta Knight levels are named
/// after what their rooms show, not after the game's chapter titles.
const LEVEL_NAMES: [&str; FIRST_UNPLAYED_LEVEL] = [
    // Spring Breeze
    "Green Greens",
    "Float Islands",
    "Bubbly Clouds",
    "Mt. Dedede",
    // Dyna Blade
    "Peanut Plains",
    "Mallow Castle",
    "Cocoa Cave",
    "Candy Mountain",
    "Dyna Blade's Nest",
    "Secret Area 1",
    "Secret Area 2",
    // Gourmet Race
    "Course 1",
    "Course 2",
    "Course 3",
    "Extra room",
    // The Great Cave Offensive
    "The Great Cave Offensive",
    "Extra room",
    // Revenge of Meta Knight
    "Prologue",
    "The shore",
    "The forest",
    "The deck",
    "The main cannon",
    "The right wing",
    "The engine room",
    "The interior",
    "The reactor",
    // Milky Way Wishes
    "Floria",
    "Hotbeat",
    "Skyhigh",
    "Cavios",
    "Aqualiss",
    "Mekkai",
    "Halfmoon",
    "???",
    "Galactic Nova",
    // The Arena
    "The Arena",
];
