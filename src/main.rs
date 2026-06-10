mod rom;
mod compression;
mod level;
mod gfx;

use eframe::egui;
use rom::Rom;
use level::{LevelManager, Room};
use std::fs;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Kirby Super Star Level Viewer",
        options,
        Box::new(|_cc| {
            Ok(Box::new(KssApp::new()))
        }),
    )
}

struct KssApp {
    rom: Option<Rom>,
    level_manager: Option<LevelManager>,
    current_room: Option<Room>,
    selected_room_idx: usize,
    error: Option<String>,
    block_size: usize,
    selected_sub_palette_idx: usize,
    use_bg_graphics: bool,
}

impl KssApp {
    fn new() -> Self {
        let mut app = Self {
            rom: None,
            level_manager: None,
            current_room: None,
            selected_room_idx: 0,
            error: None,
            block_size: 24,
            selected_sub_palette_idx: 0,
            use_bg_graphics: false,
        };

        // Load the ROM
        if let Ok(data) = fs::read("Kirby Super Star (USA).sfc") {
            let rom = Rom::new(data);
            let lm = LevelManager::new(&rom);
            app.rom = Some(rom);
            app.level_manager = Some(lm);
            app.load_current_room();
        } else {
            app.error = Some("Could not find Kirby Super Star (USA).sfc in the current directory.".to_string());
        }

        app
    }

    fn load_current_room(&mut self) {
        if let (Some(rom), Some(lm)) = (&self.rom, &self.level_manager) {
            self.current_room = lm.load_room(rom, self.selected_room_idx);
            self.selected_sub_palette_idx = 0;
            self.use_bg_graphics = false;
        }
    }
}

impl eframe::App for KssApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut new_room_idx = None;
        
        egui::Panel::left("side_panel").size_range(280.0..=360.0).show_inside(ui, |ui| {
            ui.heading("Levels & Rooms");
            ui.separator();
            
            if let Some(lm) = &self.level_manager {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for level in &lm.levels {
                        egui::CollapsingHeader::new(&level.name)
                            .default_open(level.id == 0)
                            .show(ui, |ui| {
                                for i in level.start_room_idx..level.end_room_idx {
                                    let addr = lm.room_pointers[i];
                                    let label = format!("Room {} (0x{:06X})", i, addr);
                                    if ui.selectable_label(self.selected_room_idx == i, label).clicked() {
                                        new_room_idx = Some(i);
                                    }
                                }
                            });
                    }
                });
            } else if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::RED, error);
            }
        });

        if let Some(idx) = new_room_idx {
            self.selected_room_idx = idx;
            self.load_current_room();
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let room = self.current_room.clone();
            if let Some(room) = room {
                ui.horizontal(|ui| {
                    ui.heading(format!("Room {} ({}x{})", room.id, room.width, room.height));
                    ui.label(format!("| BGM: 0x{:02X}", room.bgm_id));
                    ui.label(format!("| Scroll: {}", room.scroll_rate));
                    ui.label(format!("| Entrance: 0x{:02X} at ({}, {})", room.entrance_style, room.entrance_x, room.entrance_y));
                    ui.label(format!("| Offset: 0x{:06X}", room.pc_offset));
                });

                ui.collapsing("Raw Header Data (58 bytes)", |ui| {
                    egui::Grid::new("header_grid").show(ui, |ui| {
                        for (i, chunk) in room.header_raw.chunks(16).enumerate() {
                            ui.label(format!("0x{:02X}:", i * 16));
                            let hex: String = chunk.iter().map(|b| format!("{:02X} ", b)).collect();
                            ui.label(hex);
                            ui.end_row();
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Tile Size:");
                    ui.add(egui::Slider::new(&mut self.block_size, 8..=48));
                });

                ui.collapsing("Graphics & Palette Explorer", |ui| {
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut self.use_bg_graphics, "Show Background Graphics").changed() {
                            self.selected_sub_palette_idx = 0;
                        }
                        
                        let palette_len = if self.use_bg_graphics {
                            room.background_palette_bytes.len() / 32
                        } else {
                            room.palette_bytes.len() / 32
                        };
                        
                        if palette_len > 0 {
                            ui.label("Sub-Palette:");
                            let max_pal = palette_len.saturating_sub(1);
                            ui.add(egui::Slider::new(&mut self.selected_sub_palette_idx, 0..=max_pal));
                        }
                    });

                    // 1. Render the active 16-color palette
                    let colors = if self.use_bg_graphics {
                        room.get_background_palette_colors()
                    } else {
                        room.get_palette_colors()
                    };
                    
                    let sub_pal_idx = self.selected_sub_palette_idx;
                    let sub_pal_offset = sub_pal_idx * 16;
                    let sub_pal = if sub_pal_offset + 16 <= colors.len() {
                        &colors[sub_pal_offset..sub_pal_offset + 16]
                    } else {
                        &[]
                    };

                    if !sub_pal.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("Colors:");
                            for (c_idx, color) in sub_pal.iter().enumerate() {
                                let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                                ui.painter_at(rect).rect_filled(rect, 0.0, *color);
                                response.on_hover_text(format!("Index: {}\nRGB: ({}, {}, {})", c_idx, color.r(), color.g(), color.b()));
                            }
                        });
                    }

                    // 2. Render the tileset
                    let gfx_bytes = if self.use_bg_graphics {
                        &room.background_gfx
                    } else {
                        &room.tileset_gfx
                    };
                    
                    let total_tiles = gfx_bytes.len() / 32;
                    if total_tiles > 0 && !sub_pal.is_empty() {
                        ui.separator();
                        ui.label(format!("Tileset: {} tiles found", total_tiles));
                        
                        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                            let tile_size = 24.0; // 3x scaled for visibility
                            let columns = 16;
                            let rows = (total_tiles + columns - 1) / columns;
                            
                            let (grid_rect, _response) = ui.allocate_exact_size(
                                egui::vec2(columns as f32 * tile_size, rows as f32 * tile_size),
                                egui::Sense::hover(),
                            );
                            let painter = ui.painter_at(grid_rect);
                            
                            for t_idx in 0..total_tiles {
                                let tile_offset = t_idx * 32;
                                let tile_data = &gfx_bytes[tile_offset..tile_offset + 32];
                                
                                let grid_x = (t_idx % columns) as f32;
                                let grid_y = (t_idx / columns) as f32;
                                
                                let tile_pos = grid_rect.min + egui::vec2(grid_x * tile_size, grid_y * tile_size);
                                
                                // Draw 8x8 pixels
                                for py in 0..8 {
                                    for px in 0..8 {
                                        let c_idx = level::decode_4bpp_tile_pixel(tile_data, px, py) as usize;
                                        let color = if c_idx < sub_pal.len() {
                                            sub_pal[c_idx]
                                        } else {
                                            egui::Color32::BLACK
                                        };
                                        
                                        let px_size = tile_size / 8.0;
                                        let px_rect = egui::Rect::from_min_size(
                                            tile_pos + egui::vec2(px as f32 * px_size, py as f32 * px_size),
                                            egui::vec2(px_size, px_size),
                                        );
                                        painter.rect_filled(px_rect, 0.0, color);
                                    }
                                }
                            }
                        });
                    }
                });

                ui.separator();

                egui::ScrollArea::both().show(ui, |ui| {
                    let map_width = room.width as f32 * self.block_size as f32;
                    let map_height = room.height as f32 * self.block_size as f32;
                    
                    if map_width == 0.0 || map_height == 0.0 {
                        ui.colored_label(egui::Color32::YELLOW, "Empty or invalid room dimensions");
                        return;
                    }

                    let (rect, _response) = ui.allocate_exact_size(
                        egui::vec2(map_width, map_height),
                        egui::Sense::hover(),
                    );

                    let painter = ui.painter_at(rect);
                    
                    // Render tilemap
                    for y in 0..room.height {
                        for x in 0..room.width {
                            let idx = (y as usize * room.width as usize) + x as usize;
                            
                            if idx < room.tilemap.len() {
                                let tile = room.tilemap[idx];
                                let pos = rect.min + egui::vec2(x as f32 * self.block_size as f32, y as f32 * self.block_size as f32);
                                
                                // Generate a distinct color for each ID
                                let r = ((tile as f32 * 1.6).sin() * 127.0 + 128.0) as u8;
                                let g = ((tile as f32 * 1.1).sin() * 127.0 + 128.0) as u8;
                                let b = ((tile as f32 * 1.3).sin() * 127.0 + 128.0) as u8;
                                let color = egui::Color32::from_rgb(r, g, b);
                                
                                painter.rect_filled(
                                    egui::Rect::from_min_size(pos, egui::vec2(self.block_size as f32, self.block_size as f32)),
                                    0.0,
                                    color,
                                );
                                
                                // Draw tile ID text for larger zoom sizes
                                if self.block_size >= 20 {
                                    painter.text(
                                        pos + egui::vec2(self.block_size as f32 / 2.0, self.block_size as f32 / 2.0),
                                        egui::Align2::CENTER_CENTER,
                                        format!("{:03X}", tile),
                                        egui::FontId::proportional(self.block_size as f32 / 3.0),
                                        egui::Color32::BLACK,
                                    );
                                }
                            }
                        }
                    }

                    // Draw grid lines
                    for x in 0..=room.width {
                        let x_pos = rect.min.x + x as f32 * self.block_size as f32;
                        painter.line_segment([egui::pos2(x_pos, rect.min.y), egui::pos2(x_pos, rect.max.y)], (1.0, egui::Color32::from_gray(50)));
                    }
                    for y in 0..=room.height {
                        let y_pos = rect.min.y + y as f32 * self.block_size as f32;
                        painter.line_segment([egui::pos2(rect.min.x, y_pos), egui::pos2(rect.max.x, y_pos)], (1.0, egui::Color32::from_gray(50)));
                    }

                    // Pixel-to-display coordinate scale
                    // A metatile is 16x16 pixels.
                    let scale = self.block_size as f32 / 16.0;

                    // Render main entrance
                    let ent_pos = rect.min + egui::vec2(room.entrance_x as f32 * scale, room.entrance_y as f32 * scale);
                    painter.circle_filled(ent_pos, 8.0, egui::Color32::from_rgb(50, 120, 255));
                    painter.circle_stroke(ent_pos, 8.0, (2.0, egui::Color32::WHITE));
                    painter.text(
                        ent_pos + egui::vec2(0.0, -12.0),
                        egui::Align2::CENTER_BOTTOM,
                        "Entrance",
                        egui::FontId::proportional(12.0),
                        egui::Color32::WHITE,
                    );

                    // Render doors
                    for door in &room.doors {
                        let d_pos = rect.min + egui::vec2(door.trigger_x as f32 * scale, door.trigger_y as f32 * scale);
                        let dw = 16.0 * scale;
                        let dh = 24.0 * scale;
                        let door_rect = egui::Rect::from_center_size(d_pos, egui::vec2(dw, dh));
                        
                        painter.rect_filled(door_rect, 4.0, egui::Color32::from_rgba_unmultiplied(50, 200, 50, 180));
                        painter.rect_stroke(door_rect, 4.0, (2.0, egui::Color32::WHITE), egui::StrokeKind::Inside);
                        
                        painter.text(
                            d_pos + egui::vec2(0.0, -dh/2.0 - 4.0),
                            egui::Align2::CENTER_BOTTOM,
                            format!("Door to {}", door.dest_room),
                            egui::FontId::proportional(10.0),
                            egui::Color32::from_rgb(180, 255, 180),
                        );
                    }

                    // Render sprites
                    for sprite in &room.sprites {
                        let s_pos = rect.min + egui::vec2(sprite.x as f32 * scale, sprite.y as f32 * scale);
                        
                        painter.circle_filled(s_pos, 6.0, egui::Color32::from_rgb(255, 50, 50));
                        painter.circle_stroke(s_pos, 6.0, (2.0, egui::Color32::WHITE));
                        
                        let name = match sprite.id {
                            0x01 => "Waddle Dee",
                            0x02 => "Waddle Doo",
                            0x03 => "Sir Kibble",
                            0x06 => "Mushroom",
                            0x0A => "Flyer",
                            0x0D => "Gordo",
                            0x12 => "Knuckle Joe",
                            0x18 => "Scarfy",
                            0x1A => "Bronto Burt",
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
                            0x53 => "Kracko",
                            0x54 => "King Dedede",
                            0x55 => "Dyna Blade",
                            0x5A => "Meta Knight",
                            0x72 => "Door Star",
                            0x75 => "Warp Star",
                            0x77 => "Autoscroll",
                            0x78 => "Mine Cart",
                            0x7A => "Cannon",
                            0x80 => "1-Up",
                            0x81 => "Candy",
                            0x82 => "Maxim Tomato",
                            _ => "Enemy/Obj",
                        };
                        
                        let label = if sprite.behavior != 0 {
                            format!("{} (0x{:02X}, bhv 0x{:02X})", name, sprite.id, sprite.behavior)
                        } else {
                            format!("{} (0x{:02X})", name, sprite.id)
                        };
                        
                        painter.text(
                            s_pos + egui::vec2(8.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            label,
                            egui::FontId::proportional(11.0),
                            egui::Color32::from_rgb(255, 180, 180),
                        );
                    }
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Select a room from the side panel to view");
                });
            }
        });
    }
}
