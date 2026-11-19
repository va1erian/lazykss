//! Renders rooms, objects included, to binary PPM files: `render_room <out-dir> <room>...`.

use kss_core::{Layers, ObjectSprites, Rom, Room, Tables, Tileset};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(
        args.next()
            .ok_or("usage: render_room <out-dir> <room>...")?,
    );
    let rom = Rom::load(&Rom::locate().ok_or("ROM not found (set KSS_ROM)")?)?;
    let tables = Tables::load(&rom);
    let sprites = ObjectSprites::load(&rom);
    for arg in args {
        let index: usize = arg.parse()?;
        let room = Room::load(&rom, &tables, index)?;
        let mut image =
            Tileset::for_room(&rom, &tables, &room).render_room(&room, Layers::default());
        for object in &room.objects {
            let Some(sprite) = sprites.get(object.id, object.variant) else {
                continue;
            };
            for y in 0..sprite.image.height {
                for x in 0..sprite.image.width {
                    let pixel = sprite.image.pixel(x, y);
                    let px = object.x as i32 - sprite.origin_x + x as i32;
                    let py = object.y as i32 - sprite.origin_y + y as i32;
                    if pixel[3] != 0
                        && px >= 0
                        && py >= 0
                        && (px as usize) < image.width
                        && (py as usize) < image.height
                    {
                        let at = (py as usize * image.width + px as usize) * 4;
                        image.rgba[at..at + 4].copy_from_slice(&pixel);
                    }
                }
            }
        }
        let mut file =
            std::io::BufWriter::new(std::fs::File::create(out.join(format!("room{index}.ppm")))?);
        write!(file, "P6\n{} {}\n255\n", image.width, image.height)?;
        for pixel in image.rgba.chunks_exact(4) {
            file.write_all(&pixel[..3])?;
        }
        println!(
            "room {index}: {}x{} cells, {} objects",
            room.width,
            room.height,
            room.objects.len()
        );
    }
    Ok(())
}
