//! Renders rooms to binary PPM files: `render_room <out-dir> <room>...`.

use kss_core::{Layers, Rom, Room, Tables, Tileset};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(args.next().ok_or("usage: render_room <out-dir> <room>...")?);
    let rom = Rom::load(&Rom::locate().ok_or("ROM not found (set KSS_ROM)")?)?;
    let tables = Tables::load(&rom);
    for arg in args {
        let index: usize = arg.parse()?;
        let room = Room::load(&rom, &tables, index)?;
        let image = Tileset::for_room(&rom, &tables, &room).render_room(&room, Layers::default());
        let mut file = std::io::BufWriter::new(std::fs::File::create(out.join(format!("room{index}.ppm")))?);
        write!(file, "P6\n{} {}\n255\n", image.width, image.height)?;
        for pixel in image.rgba.chunks_exact(4) {
            file.write_all(&pixel[..3])?;
        }
        println!("room {index}: {}x{} cells, {} objects", room.width, room.height, room.objects.len());
    }
    Ok(())
}
