//! Applies edits to one room and writes a new ROM:
//!
//! ```text
//! patch_room <out.sfc> <room> [tile X Y ID]... [object ID VARIANT X Y]...
//!            [block X Y AFTER EXTRA]... [door DEST X Y DEST_X DEST_Y]...
//!            [clear-doors] [clear-objects] [slots HEX8] [raw OFFSET HEX] [noise]
//! ```
//!
//! `door` appends a door record (`DEST` counts from the level's first room);
//! `clear-doors` empties the list.
//!
//! `slots` replaces the four object-graphics slot bytes of the graphics
//! header; `noise` fills the unused right part of the map with
//! incompressible tiles so the room has to move to free space.

use kss_core::{Block, Door, Object, Rom, Room, Tables};
use std::error::Error;
use std::slice::Iter;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The next argument of `edit`.
fn text<'a>(args: &mut Iter<'a, String>, edit: &str) -> Result<&'a str> {
    Ok(args
        .next()
        .ok_or_else(|| format!("{edit}: missing argument"))?)
}

/// The next argument of `edit` as a number, decimal or `0x` hex.
fn number(args: &mut Iter<'_, String>, edit: &str) -> Result<usize> {
    let s = text(args, edit)?;
    Ok(match s.strip_prefix("0x") {
        Some(hex) => usize::from_str_radix(hex, 16)?,
        None => s.parse()?,
    })
}

/// Bytes written as hex digit pairs; a trailing odd digit is ignored.
fn hex_bytes(hex: &str) -> Result<Vec<u8>> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| Ok(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?))
        .collect()
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: patch_room <out.sfc> <room> [edits...]";
    let out = std::path::PathBuf::from(args.first().ok_or(usage)?);
    let index: usize = args.get(1).ok_or(usage)?.parse()?;
    let mut rom = Rom::load(&Rom::locate().ok_or("ROM not found (set KSS_ROM)")?)?;
    let mut tables = Tables::load(&rom);
    let mut room = Room::load(&rom, &tables, index)?;
    let mut rest = args.get(2..).unwrap_or_default().iter();
    while let Some(edit) = rest.next() {
        let args = &mut rest;
        match edit.as_str() {
            "tile" => {
                let (x, y, id) = (
                    number(args, edit)?,
                    number(args, edit)?,
                    number(args, edit)?,
                );
                room.set_tile(x, y, id as u16);
            }
            "object" => {
                room.objects.push(Object {
                    id: number(args, edit)? as u8,
                    variant: number(args, edit)? as u8,
                    x: number(args, edit)? as u16,
                    y: number(args, edit)? as u16,
                });
            }
            "block" => {
                // block X Y AFTER EXTRA: make the cell breakable.
                let mut blocks = room.blocks();
                blocks.push(Block {
                    x: number(args, edit)?,
                    y: number(args, edit)?,
                    after: number(args, edit)? as u16,
                    extra: number(args, edit)? as u16,
                });
                room.set_blocks(&blocks);
            }
            "door" => {
                let mut n = || number(args, edit).map(|v| v as u16);
                let door = Door {
                    dest_room: n()?,
                    x: n()?,
                    y: n()?,
                    dest_x: n()?,
                    dest_y: n()?,
                };
                let count = room.doors().len();
                if !room.insert_door(count, door) {
                    return Err(format!("door {door:?} cannot be added").into());
                }
            }
            "clear-doors" => room.set_doors(&[]),
            "clear-objects" => room.objects.clear(),
            "slots" => {
                let value = u32::from_str_radix(text(args, edit)?, 16)?;
                let mut header = room.gfx_header();
                header[20..24].copy_from_slice(&value.to_be_bytes());
                room.set_gfx_header(header);
            }
            "raw" => {
                // raw OFFSET HEXBYTES: overwrite header bytes of the room block.
                let offset = number(args, edit)?;
                let bytes = hex_bytes(text(args, edit)?)?;
                room.patch_head(offset, &bytes);
            }
            "noise" => {
                let mut seed = 99u32;
                for y in 0..room.height.saturating_sub(2) {
                    for x in room.width * 3 / 4..room.width {
                        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                        room.set_tile(x, y, 1 + (seed >> 16) as u16 % 200);
                    }
                }
            }
            other => return Err(format!("unknown edit {other:?}").into()),
        }
    }
    let report = room.save(&mut rom, &mut tables)?;
    rom.save(&out)?;
    println!("{report:?}");
    Ok(())
}
