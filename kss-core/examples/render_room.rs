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
                    // `put` drops pixels outside the image.
                    if let (Ok(px), Ok(py)) = (usize::try_from(px), usize::try_from(py))
                        && pixel[3] != 0
                    {
                        image.put(px, py, pixel);
                    }
                }
            }
        }
        let mut file =
            std::io::BufWriter::new(std::fs::File::create(out.join(format!("room{index}.ppm")))?);
        write!(file, "P6\n{} {}\n255\n", image.width, image.height)?;
        let rgb: Vec<u8> = image
            .rgba
            .chunks_exact(4)
            .flat_map(|pixel| pixel.iter().take(3))
            .copied()
            .collect();
        file.write_all(&rgb)?;
        println!(
            "room {index}: {}x{} cells, {} objects",
            room.width,
            room.height,
            room.objects.len()
        );
    }
    Ok(())
}
