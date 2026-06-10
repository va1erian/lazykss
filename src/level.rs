use crate::rom::Rom;
use crate::compression::Decompressor;
use eframe::egui;

#[derive(Debug, Clone)]
pub struct Door {
    pub dest_room: u16,
    pub trigger_x: u16,
    pub trigger_y: u16,
    #[allow(dead_code)]
    pub exit_x: u16,
    #[allow(dead_code)]
    pub exit_y: u16,
}

#[derive(Debug, Clone)]
pub struct Sprite {
    pub id: u8,
    pub behavior: u8,
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Clone)]
pub struct Level {
    pub id: usize,
    pub name: String,
    pub start_room_idx: usize,
    pub end_room_idx: usize,
}

#[derive(Debug, Clone)]
pub struct Room {
    pub id: usize,
    pub pc_offset: usize,
    pub width: u16,
    pub height: u16,
    pub bgm_id: u8,
    pub scroll_rate: u8,
    pub entrance_style: u8,
    pub entrance_x: u16,
    pub entrance_y: u16,
    pub tilemap: Vec<u16>,
    pub sprites: Vec<Sprite>,
    pub doors: Vec<Door>,
    pub header_raw: Vec<u8>,
    
    // Extracted graphics and palettes
    pub tileset_gfx: Vec<u8>,
    pub palette_bytes: Vec<u8>,
    pub background_gfx: Vec<u8>,
    pub background_palette_bytes: Vec<u8>,
}

impl Room {
    pub fn get_palette_colors(&self) -> Vec<egui::Color32> {
        let mut colors = Vec::new();
        for chunk in self.palette_bytes.chunks_exact(2) {
            let val = u16::from_le_bytes([chunk[0], chunk[1]]);
            colors.push(snes_color_to_rgb(val));
        }
        colors
    }

    pub fn get_background_palette_colors(&self) -> Vec<egui::Color32> {
        let mut colors = Vec::new();
        for chunk in self.background_palette_bytes.chunks_exact(2) {
            let val = u16::from_le_bytes([chunk[0], chunk[1]]);
            colors.push(snes_color_to_rgb(val));
        }
        colors
    }
}

pub fn snes_color_to_rgb(val: u16) -> egui::Color32 {
    let r5 = val & 0x1F;
    let g5 = (val >> 5) & 0x1F;
    let b5 = (val >> 10) & 0x1F;
    let r = ((r5 << 3) | (r5 >> 2)) as u8;
    let g = ((g5 << 3) | (g5 >> 2)) as u8;
    let b = ((b5 << 3) | (b5 >> 2)) as u8;
    egui::Color32::from_rgb(r, g, b)
}

pub fn decode_4bpp_tile_pixel(tile_data: &[u8], x: usize, y: usize) -> u8 {
    if tile_data.len() < 32 { return 0; }
    let b0 = tile_data[y * 2];
    let b1 = tile_data[y * 2 + 1];
    let b2 = tile_data[16 + y * 2];
    let b3 = tile_data[16 + y * 2 + 1];
    
    let shift = 7 - x;
    let bit0 = (b0 >> shift) & 1;
    let bit1 = (b1 >> shift) & 1;
    let bit2 = (b2 >> shift) & 1;
    let bit3 = (b3 >> shift) & 1;
    
    bit0 | (bit1 << 1) | (bit2 << 2) | (bit3 << 3)
}

pub struct LevelManager {
    pub room_pointers: Vec<usize>,
    pub levels: Vec<Level>,
}

impl LevelManager {
    pub fn new(rom: &Rom) -> Self {
        let mut room_pointers = Vec::new();
        let mut levels = Vec::new();
        
        // 1. Read the start of the room pointer table from $FF:0000 (offset in bank FF)
        let room_ptr_table_offset_in_ff = rom.read_u16(0xFF, 0x0000) as usize;
        let room_ptr_table_pc = 0x3F0000 + room_ptr_table_offset_in_ff;
        
        // 2. Read the start of the graphics table from $FF:0002 (offset in bank FF)
        let gfx_ptr_table_offset_in_ff = rom.read_u16(0xFF, 0x0002) as usize;
        let gfx_ptr_table_pc = 0x3F0000 + gfx_ptr_table_offset_in_ff;
        
        // 3. Compute number of rooms
        let num_rooms = (gfx_ptr_table_pc - room_ptr_table_pc) / 4;
        
        // 4. Read room pointer array (4-byte pointers: 3 bytes address, 1 byte padding)
        for i in 0..num_rooms {
            let ptr_pc = room_ptr_table_pc + i * 4;
            let b0 = rom.read_u8_at_pc(ptr_pc) as usize;
            let b1 = rom.read_u8_at_pc(ptr_pc + 1) as usize;
            let b2 = rom.read_u8_at_pc(ptr_pc + 2) as usize;
            let snes_long = b0 | (b1 << 8) | (b2 << 16);
            
            let pc_offset = if snes_long >= 0xC00000 {
                snes_long - 0xC00000
            } else {
                let bank = (snes_long >> 16) & 0xFF;
                let addr = snes_long & 0xFFFF;
                ((bank & 0x7F) * 0x8000) | (addr & 0x7FFF)
            };
            room_pointers.push(pc_offset);
        }

        // 5. Read the level pointer table from $FF:000C (PC $3F0022)
        let level_table_offset_in_ff = rom.read_u16(0xFF, 0x000C) as usize;
        let level_table_pc = 0x3F0000 + level_table_offset_in_ff;
        
        let mut level_starts = Vec::new();
        for i in 0..60 {
            let entry_pc = level_table_pc + i * 2;
            let val = rom.read_u16_at_pc(entry_pc) as usize;
            let room_idx = (val - room_ptr_table_offset_in_ff) / 4;
            level_starts.push(room_idx);
        }

        for i in 0..60 {
            let start = level_starts[i];
            let end = if i + 1 < 60 { level_starts[i+1] } else { num_rooms };
            
            let name = format!("Level {:02}", i);
            levels.push(Level {
                id: i,
                name,
                start_room_idx: start,
                end_room_idx: end,
            });
        }

        Self { room_pointers, levels }
    }

    #[allow(dead_code)]
    pub fn get_room_count(&self) -> usize {
        self.room_pointers.len()
    }

    pub fn load_room(&self, rom: &Rom, room_index: usize) -> Option<Room> {
        if room_index >= self.room_pointers.len() { return None; }
        
        let start_addr = self.room_pointers[room_index];
        
        if start_addr + 58 > rom.data().len() { return None; }

        // 1. Read headers
        let misc_header = rom.read_slice_at_pc(start_addr, 17);
        let bgm_id = misc_header[2];
        let scroll_rate = misc_header[6];
        let entrance_style = misc_header[12];
        
        // Entrance position is 4 bytes, starts at start_addr + 54
        let entrance_bytes = rom.read_slice_at_pc(start_addr + 54, 4);
        let entrance_x = u16::from_le_bytes([entrance_bytes[0], entrance_bytes[1]]);
        let entrance_y = u16::from_le_bytes([entrance_bytes[2], entrance_bytes[3]]);

        // 2. Read graphics and palettes
        let tileset_gfx_idx = rom.read_u8_at_pc(start_addr + 24) as usize;
        let room_pal_idx = rom.read_u8_at_pc(start_addr + 25) as usize;
        let bg_gfx_idx = rom.read_u8_at_pc(start_addr + 29) as usize;
        let bg_pal_idx = rom.read_u8_at_pc(start_addr + 30) as usize;

        let gfx_table_offset_in_ff = rom.read_u16(0xFF, 0x0002) as usize;
        let gfx_table_pc = 0x3F0000 + gfx_table_offset_in_ff;

        let load_gfx_entry = |idx: usize| -> Vec<u8> {
            let ptr_pc = gfx_table_pc + idx * 3;
            if ptr_pc + 3 > rom.data().len() {
                return Vec::new();
            }
            let b0 = rom.read_u8_at_pc(ptr_pc) as usize;
            let b1 = rom.read_u8_at_pc(ptr_pc + 1) as usize;
            let b2 = rom.read_u8_at_pc(ptr_pc + 2) as usize;
            let snes_long = b0 | (b1 << 8) | (b2 << 16);
            
            let pc_offset = if snes_long >= 0xC00000 {
                snes_long - 0xC00000
            } else {
                let bank = (snes_long >> 16) & 0xFF;
                let addr = snes_long & 0xFFFF;
                ((bank & 0x7F) * 0x8000) | (addr & 0x7FFF)
            };
            if pc_offset >= rom.data().len() {
                return Vec::new();
            }
            let decompressor = Decompressor::new(&rom.data()[pc_offset..]);
            decompressor.decompress()
        };

        let tileset_gfx = load_gfx_entry(tileset_gfx_idx);
        let palette_bytes = load_gfx_entry(room_pal_idx);
        let background_gfx = load_gfx_entry(bg_gfx_idx);
        let background_palette_bytes = load_gfx_entry(bg_pal_idx);

        // Doors:
        // Section 0 starts at start_addr + 58
        let sec0_rel = rom.read_u16_at_pc(start_addr + 58) as usize;
        let sec0_end = start_addr + 58 + sec0_rel;
        
        let mut doors = Vec::new();
        let mut door_ptr = start_addr + 60;
        
        while door_ptr < sec0_end {
            let val = rom.read_u8_at_pc(door_ptr);
            if val == 0xFF {
                break;
            }
            if door_ptr + 10 > sec0_end {
                break;
            }
            
            let dest_room = rom.read_u16_at_pc(door_ptr);
            let trigger_x = rom.read_u16_at_pc(door_ptr + 2);
            let trigger_y = rom.read_u16_at_pc(door_ptr + 4);
            let exit_x = rom.read_u16_at_pc(door_ptr + 6);
            let exit_y = rom.read_u16_at_pc(door_ptr + 8);
            
            doors.push(Door {
                dest_room,
                trigger_x,
                trigger_y,
                exit_x,
                exit_y,
            });
            
            door_ptr += 10;
        }

        // Walk sections to get dimensions
        let mut current_sec_pc = start_addr + 58;
        for _ in 0..6 {
            if current_sec_pc + 2 > rom.data().len() { return None; }
            let rel_ptr = rom.read_u16_at_pc(current_sec_pc) as usize;
            current_sec_pc += rel_ptr;
        }
        
        if current_sec_pc + 6 > rom.data().len() { return None; }
        
        let width = rom.read_u16_at_pc(current_sec_pc + 2);
        let height = rom.read_u16_at_pc(current_sec_pc + 4);
        
        let comp_data_start = current_sec_pc + 6;
        if comp_data_start >= rom.data().len() { return None; }
        
        let decompressor = Decompressor::new(&rom.data()[comp_data_start..]);
        let decompressed = decompressor.decompress();

        let mut tilemap = Vec::new();
        let mut sprites = Vec::new();

        let map_size = (width as usize) * (height as usize);
        if decompressed.len() >= 2 * map_size {
            // Read tile ID planes
            for i in 0..map_size {
                let low = decompressed[i] as u16;
                let high = decompressed[i + map_size] as u16;
                tilemap.push(low | (high << 8));
            }
            
            // Read sprite list
            let mut sprite_ptr = 2 * map_size;
            while sprite_ptr + 6 <= decompressed.len() {
                if decompressed[sprite_ptr] == 0xFF && decompressed[sprite_ptr + 1] == 0xFF {
                    break;
                }
                
                let behavior = decompressed[sprite_ptr];
                let id = decompressed[sprite_ptr + 1];
                let x = u16::from_le_bytes([decompressed[sprite_ptr + 2], decompressed[sprite_ptr + 3]]);
                let y = u16::from_le_bytes([decompressed[sprite_ptr + 4], decompressed[sprite_ptr + 5]]);
                
                sprites.push(Sprite {
                    id,
                    behavior,
                    x,
                    y,
                });
                
                sprite_ptr += 6;
            }
        }

        let header_raw = rom.data()[start_addr..start_addr + 58].to_vec();

        Some(Room {
            id: room_index,
            pc_offset: start_addr,
            width,
            height,
            bgm_id,
            scroll_rate,
            entrance_style,
            entrance_x,
            entrance_y,
            tilemap,
            sprites,
            doors,
            header_raw,
            tileset_gfx,
            palette_bytes,
            background_gfx,
            background_palette_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snes_color_to_rgb() {
        let c_red = snes_color_to_rgb(0x001F);
        assert_eq!(c_red, egui::Color32::from_rgb(255, 0, 0));
        
        let c_green = snes_color_to_rgb(0x03E0);
        assert_eq!(c_green, egui::Color32::from_rgb(0, 255, 0));
        
        let c_blue = snes_color_to_rgb(0x7C00);
        assert_eq!(c_blue, egui::Color32::from_rgb(0, 0, 255));
    }

    #[test]
    fn test_decode_4bpp_tile_pixel() {
        let mut tile_data = vec![0u8; 32];
        tile_data[0] = 0x80;
        tile_data[1] = 0x80;
        tile_data[16] = 0x80;
        tile_data[17] = 0x80;
        
        assert_eq!(decode_4bpp_tile_pixel(&tile_data, 0, 0), 15);
        assert_eq!(decode_4bpp_tile_pixel(&tile_data, 1, 0), 0);
    }

    #[test]
    fn test_load_room_1_green_greens() {
        if let Ok(data) = std::fs::read("Kirby Super Star (USA).sfc") {
            let rom = Rom::new(data);
            let lm = LevelManager::new(&rom);
            
            // Validate room pointer count is 319
            assert_eq!(lm.room_pointers.len(), 319);
            
            // Load Green Greens room 1
            let room = lm.load_room(&rom, 1).expect("Should load Room 1");
            
            assert_eq!(room.id, 1);
            assert_eq!(room.pc_offset, 0x3020EF);
            assert_eq!(room.width, 100);
            assert_eq!(room.height, 8);
            assert_eq!(room.bgm_id, 0x0F);
            assert_eq!(room.scroll_rate, 0x13);
            assert_eq!(room.entrance_style, 0x00);
            assert_eq!(room.entrance_x, 43);
            assert_eq!(room.entrance_y, 42);
            
            // GFX and Palette indices from Room Header (24 bytes @ start_addr + 22)
            // Offset 24 is tileset_gfx_idx
            assert_eq!(rom.read_u8_at_pc(room.pc_offset + 24), 0x08);
            // Offset 25 is room_pal_idx
            assert_eq!(rom.read_u8_at_pc(room.pc_offset + 25), 0x6A);
            // Offset 29 is bg_gfx_idx
            assert_eq!(rom.read_u8_at_pc(room.pc_offset + 29), 0x2B);
            // Offset 30 is bg_pal_idx
            assert_eq!(rom.read_u8_at_pc(room.pc_offset + 30), 0x40);

            // Check if decompression loaded data
            assert!(!room.tileset_gfx.is_empty(), "tileset_gfx should be populated");
            assert!(!room.palette_bytes.is_empty(), "palette_bytes should be populated");
            assert!(!room.background_gfx.is_empty(), "background_gfx should be populated");
            assert!(!room.background_palette_bytes.is_empty(), "background_palette_bytes should be populated");

            // Doors check: Room 1 has 2 doors
            assert_eq!(room.doors.len(), 2);
            
            // Door 1
            assert_eq!(room.doors[0].dest_room, 2);
            assert_eq!(room.doors[0].trigger_x, 0x0180);
            assert_eq!(room.doors[0].trigger_y, 0x0078);
            assert_eq!(room.doors[0].exit_x, 0x009C);
            assert_eq!(room.doors[0].exit_y, 0x006C);
            
            // Door 2
            assert_eq!(room.doors[1].dest_room, 3);
            assert_eq!(room.doors[1].trigger_x, 0xFF00);
            assert_eq!(room.doors[1].trigger_y, 0xFF00);
            assert_eq!(room.doors[1].exit_x, 0x00C0);
            assert_eq!(room.doors[1].exit_y, 0x0078);

            // Sprites check: Room 1 has 8 sprites
            assert_eq!(room.sprites.len(), 8);
            
            // Sprite 0: Door Star (0x72)
            assert_eq!(room.sprites[0].id, 0x72);
            // Sprite 1: Warp Star (0x75)
            assert_eq!(room.sprites[1].id, 0x75);
            // Sprite 2: Bronto Burt (0x1A) at x=0x022D, y=0x0074
            let bronto_burt = &room.sprites[2];
            assert_eq!(bronto_burt.id, 0x1A);
            assert_eq!(bronto_burt.behavior, 0x00);
            assert_eq!(bronto_burt.x, 0x022D);
            assert_eq!(bronto_burt.y, 0x0074);
        } else {
            panic!("Kirby Super Star (USA).sfc not found! Test cannot run.");
        }
    }
}
