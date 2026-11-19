//! Applies edits to one room and writes a new ROM:
//!
//! ```text
//! patch_room <out.sfc> <room> [tile X Y ID]... [object ID VARIANT X Y]...
//!            [block X Y AFTER EXTRA]... [clear-objects] [slots HEX8] [raw OFFSET HEX] [noise]
//! ```
//!
//! `slots` replaces the four object-graphics slot bytes of the graphics
//! header; `noise` fills the unused right part of the map with
//! incompressible tiles so the room has to move to free space.

use kss_core::{Block, Object, Rom, Room, Tables};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: patch_room <out.sfc> <room> [edits...]";
    let out = std::path::PathBuf::from(args.first().ok_or(usage)?);
    let index: usize = args.get(1).ok_or(usage)?.parse()?;
    let mut rom = Rom::load(&Rom::locate().ok_or("ROM not found (set KSS_ROM)")?)?;
    let mut tables = Tables::load(&rom);
    let mut room = Room::load(&rom, &tables, index)?;
    let number = |s: &String| -> Result<usize, std::num::ParseIntError> {
        match s.strip_prefix("0x") {
            Some(hex) => usize::from_str_radix(hex, 16),
            None => s.parse(),
        }
    };
    let mut at = 2;
    while at < args.len() {
        match args[at].as_str() {
            "tile" => {
                let (x, y, id) = (number(&args[at + 1])?, number(&args[at + 2])?, number(&args[at + 3])?);
                room.set_tile(x, y, id as u16);
                at += 4;
            }
            "object" => {
                room.objects.push(Object {
                    id: number(&args[at + 1])? as u8,
                    variant: number(&args[at + 2])? as u8,
                    x: number(&args[at + 3])? as u16,
                    y: number(&args[at + 4])? as u16,
                });
                at += 5;
            }
            "block" => {
                // block X Y AFTER EXTRA: make the cell breakable.
                let mut blocks = room.blocks();
                blocks.push(Block {
                    x: number(&args[at + 1])?,
                    y: number(&args[at + 2])?,
                    after: number(&args[at + 3])? as u16,
                    extra: number(&args[at + 4])? as u16,
                });
                room.set_blocks(&blocks);
                at += 5;
            }
            "clear-objects" => {
                room.objects.clear();
                at += 1;
            }
            "slots" => {
                let value = u32::from_str_radix(&args[at + 1], 16)?;
                let mut header = room.gfx_header();
                header[20..24].copy_from_slice(&value.to_be_bytes());
                room.set_gfx_header(header);
                at += 2;
            }
            "raw" => {
                // raw OFFSET HEXBYTES: overwrite header bytes of the room block.
                let offset = number(&args[at + 1])?;
                let bytes: Vec<u8> = (0..args[at + 2].len() / 2)
                    .map(|i| u8::from_str_radix(&args[at + 2][i * 2..i * 2 + 2], 16))
                    .collect::<Result<_, _>>()?;
                room.patch_head(offset, &bytes);
                at += 3;
            }
            "noise" => {
                let mut seed = 99u32;
                for y in 0..room.height.saturating_sub(2) {
                    for x in room.width * 3 / 4..room.width {
                        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                        room.set_tile(x, y, 1 + (seed >> 16) as u16 % 200);
                    }
                }
                at += 1;
            }
            other => return Err(format!("unknown edit {other:?}").into()),
        }
    }
    let report = room.save(&mut rom, &mut tables)?;
    rom.save(&out)?;
    println!("{report:?}");
    Ok(())
}
