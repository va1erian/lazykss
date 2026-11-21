//! Kirby Super Star (USA) data formats: ROM access, the HAL codec, rooms,
//! tilesets and the save pipeline. No UI dependencies.
//!
//! Every byte read from the ROM is untrusted, so arithmetic on it is checked,
//! saturating or wrapping by choice, never a debug-build overflow panic.

#![cfg_attr(not(test), deny(clippy::arithmetic_side_effects))]

pub mod compression;
pub mod gfx;
pub mod names;
pub mod rom;
pub mod room;
mod sprite_table;
pub mod sprites;
pub mod treasure;

pub use gfx::{Image, Layers, Tileset};
pub use rom::Rom;
pub use room::{Block, CELL, Destination, Door, Level, Object, Room, SaveReport, SubGame, Tables};
pub use sprites::{ObjectSprites, Sprite};
