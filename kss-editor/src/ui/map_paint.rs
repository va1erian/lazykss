//! Painting the map: room image, grid, doors, objects and tool overlays.

use kss_core::names::object_name;
use kss_core::{CELL, Object};
use xui::Color;
use xui::Dip;
use xui::backend::{Canvas, Rgba, TextStyle};
use xui::geometry::{Point, Rect};
use xui::theme::Theme;

use crate::editor::{Editor, Tool};

use super::render::{CHUNK, ChunkCache};
use super::view::ViewState;

const TEXT: Dip = Dip(10.0);

/// Paints the map into `canvas` (the node's own canvas).
pub fn paint(
    canvas: &mut dyn Canvas,
    editor: &Option<Editor>,
    view: &mut ViewState,
    cache: &mut ChunkCache,
    theme: &Theme,
) {
    let origin = canvas.bounds();
    canvas.save();
    canvas.set_translation(origin.left as f32, origin.top as f32);
    let bounds = Rect::from_size(origin.size());
    view.scroller.size = (bounds.width(), bounds.height());
    view.scroller.dpi = canvas.dpi();
    canvas.fill_rect(bounds, theme.background);
    match editor {
        Some(editor) => paint_room(canvas, editor, view, cache, theme),
        None => {
            // The empty state: a button-like card; any click on the map opens the dialog.
            let (cx, cy) = (bounds.width() / 2, bounds.height() / 2);
            let card = Rect::new(cx - 110, cy - 22, cx + 110, cy + 22);
            canvas.fill_rounded_rect(card, 6.0, theme.accent);
            let style = TextStyle::new(theme.text_on_accent, Dip(14.0))
                .centered()
                .middle();
            canvas.draw_text("Open ROM...", card, &style);
            let hint = TextStyle::new(theme.text_secondary, Dip(12.0)).centered();
            let below = Rect::new(0, card.bottom + 12, bounds.width(), card.bottom + 40);
            canvas.draw_text(
                "No Kirby Super Star (USA) ROM found. Click here, or use File > Open ROM.",
                below,
                &hint,
            );
        }
    }
    canvas.restore();
}

fn cache_key(editor: &Editor, view: &ViewState) -> u64 {
    let layers = view.layers.background as u64 | (view.layers.foreground as u64) << 1;
    (editor.current_index() as u64) << 48 | (editor.revision() & 0xFFFF_FFFF) << 2 | layers
}

fn paint_room(
    canvas: &mut dyn Canvas,
    editor: &Editor,
    view: &mut ViewState,
    cache: &mut ChunkCache,
    theme: &Theme,
) {
    let room = editor.room();
    view.set_room(room.width, room.height);

    let key = cache_key(editor, view);
    if cache.key != key {
        cache.set_source(key, editor.tileset().render_room(room, view.layers));
    }

    let area = view.scroller.view();
    let (zoom, (ox, oy)) = (view.zoom, view.scroller.offset);
    canvas.push_clip(area);

    // Room image, chunk by chunk.
    let (image_w, image_h) = (room.width * CELL, room.height * CELL);
    let chunk_px = CHUNK as i32 * zoom;
    let first = (ox.max(0) / chunk_px) as usize;
    let last = ((ox + area.width()).max(0) / chunk_px) as usize;
    let first_y = (oy.max(0) / chunk_px) as usize;
    let last_y = ((oy + area.height()).max(0) / chunk_px) as usize;
    for cy in first_y..=last_y {
        for cx in first..=last {
            if cx * CHUNK >= image_w || cy * CHUNK >= image_h {
                continue;
            }
            if let Some(chunk) = cache.chunk(cx, cy, zoom as usize) {
                let left = cx as i32 * chunk_px - ox;
                let top = cy as i32 * chunk_px - oy;
                canvas.draw_image(
                    chunk,
                    Rect::new(
                        left,
                        top,
                        left + chunk.width() as i32,
                        top + chunk.height() as i32,
                    ),
                );
            }
        }
    }

    if view.grid {
        paint_grid(canvas, view, room.width, room.height, area);
    }
    if view.doors {
        paint_doors(canvas, editor, view, theme);
    }
    if view.objects {
        paint_objects(canvas, editor, view, cache, theme);
    }
    paint_tool_overlay(canvas, editor, view, cache, theme);

    canvas.pop_clip();
    view.scroller.paint(canvas, theme);
}

fn paint_grid(canvas: &mut dyn Canvas, view: &ViewState, width: usize, height: usize, area: Rect) {
    let cell = CELL as i32 * view.zoom;
    let line = Rgba::with_alpha(255, 255, 255, 70);
    let (ox, oy) = view.scroller.offset;
    let (w, h) = (width as i32 * cell, height as i32 * cell);
    for i in (ox / cell)..=((ox + area.width()) / cell).min(width as i32) {
        let x = i * cell - ox;
        canvas.fill_rect_rgba(Rect::new(x, 0, x + 1, (h - oy).min(area.bottom)), line);
    }
    for j in (oy / cell)..=((oy + area.height()) / cell).min(height as i32) {
        let y = j * cell - oy;
        canvas.fill_rect_rgba(Rect::new(0, y, (w - ox).min(area.right), y + 1), line);
    }
}

/// A short text on a dark pill, `anchor` being its top-left corner.
fn label(canvas: &mut dyn Canvas, text: &str, anchor: Point, color: Color) {
    let style = TextStyle::new(color, TEXT);
    let m = canvas.measure_text(text, &style);
    let rect = Rect::new(
        anchor.x,
        anchor.y,
        anchor.x + m.width + 6,
        anchor.y + m.height + 2,
    );
    canvas.fill_rect_rgba(rect, Rgba::with_alpha(0, 0, 0, 170));
    canvas.draw_text(
        text,
        Rect::new(rect.left + 3, rect.top + 1, rect.right, rect.bottom),
        &style,
    );
}

fn paint_doors(canvas: &mut dyn Canvas, editor: &Editor, view: &ViewState, theme: &Theme) {
    let side = CELL as i32 * view.zoom;
    for (i, door) in editor.room().doors().iter().enumerate() {
        if !door.has_trigger() {
            continue;
        }
        let (cx, cy) = view.to_screen((door.x as i32, door.y as i32));
        let rect = Rect::new(cx - side / 2, cy - side / 2, cx + side / 2, cy + side / 2);
        let selected = editor.selected_door == Some(i);
        let colour = Color::rgb(0xFF, 0xB0, 0x20);
        canvas.fill_rect_rgba(
            rect,
            Rgba::with_alpha(0xFF, 0xB0, 0x20, if selected { 110 } else { 60 }),
        );
        canvas.stroke_rect(
            rect,
            if selected {
                theme.text_on_accent
            } else {
                colour
            },
            if selected { 3.0 } else { 2.0 },
        );
        label(
            canvas,
            &format!("\u{2192} room {}", door.dest_room),
            Point::new(rect.left, rect.top - 18),
            colour,
        );
    }
}

/// Box colour by object family.
fn object_colour(id: u8) -> Color {
    match id {
        0x80..=0x9F => Color::rgb(0x40, 0xD0, 0x60),
        0xA0..=0xFF => Color::rgb(0xC0, 0x80, 0xFF),
        _ => Color::rgb(0xFF, 0x50, 0x50),
    }
}

/// Draws an object at screen position `centre`: its sprite when the ROM
/// table has one, else a box with its id. Returns the rectangle it covers.
fn paint_object_body(
    canvas: &mut dyn Canvas,
    editor: &Editor,
    cache: &mut ChunkCache,
    object: &Object,
    centre: (i32, i32),
    zoom: i32,
) -> Rect {
    let (cx, cy) = centre;
    let colour = object_colour(object.id);
    let origin = editor
        .sprites()
        .get(object.id, object.variant)
        .map(|s| (s.origin_x, s.origin_y));
    let icon = cache.sprite(editor.sprites(), object.id, object.variant, zoom as usize);
    if let (Some(icon), Some((ox, oy))) = (icon, origin) {
        let (left, top) = (cx - ox * zoom, cy - oy * zoom);
        let rect = Rect::new(
            left,
            top,
            left + icon.width() as i32,
            top + icon.height() as i32,
        );
        canvas.draw_image(icon, rect);
        rect
    } else {
        let half = (8 * zoom).max(8);
        let rect = Rect::new(cx - half, cy - half, cx + half, cy + half);
        canvas.fill_rect_rgba(rect, Rgba::with_alpha(colour.r, colour.g, colour.b, 110));
        canvas.stroke_rect(rect, colour, 2.0);
        let style = TextStyle::new(Color::rgb(255, 255, 255), TEXT)
            .bold()
            .centered()
            .middle();
        canvas.draw_text(&format!("{:02X}", object.id), rect, &style);
        rect
    }
}

fn paint_objects(
    canvas: &mut dyn Canvas,
    editor: &Editor,
    view: &ViewState,
    cache: &mut ChunkCache,
    theme: &Theme,
) {
    let area = view.scroller.view();
    for (i, object) in editor.room().objects.iter().enumerate() {
        let centre = view.to_screen((object.x as i32, object.y as i32));
        if centre.0 < -80
            || centre.1 < -80
            || centre.0 > area.right + 80
            || centre.1 > area.bottom + 80
        {
            continue;
        }
        let selected = editor.selected_object == Some(i);
        let rect = paint_object_body(canvas, editor, cache, object, centre, view.zoom);
        if selected {
            canvas.stroke_rect(
                Rect::new(rect.left - 3, rect.top - 3, rect.right + 3, rect.bottom + 3),
                theme.accent,
                3.0,
            );
        }
        // A sprite speaks for itself; a box, or the selection, gets its name.
        let has_sprite = editor.sprites().get(object.id, object.variant).is_some();
        if selected || !has_sprite {
            let name = object_name(object.id)
                .map_or_else(|| format!("object {:02X}", object.id), str::to_string);
            label(
                canvas,
                &name,
                Point::new(rect.left, rect.bottom + 2),
                Color::rgb(255, 255, 255),
            );
        }
    }
}

fn cell_rect(view: &ViewState, cell: (i32, i32)) -> Rect {
    let (x, y) = view.to_screen((cell.0 * CELL as i32, cell.1 * CELL as i32));
    let side = CELL as i32 * view.zoom;
    Rect::new(x, y, x + side, y + side)
}

fn paint_tool_overlay(
    canvas: &mut dyn Canvas,
    editor: &Editor,
    view: &ViewState,
    cache: &mut ChunkCache,
    theme: &Theme,
) {
    let accent = theme.accent;
    if let Some(((x0, y0), (x1, y1))) = view.rect_drag {
        let a = cell_rect(view, (x0.min(x1), y0.min(y1)));
        let b = cell_rect(view, (x0.max(x1), y0.max(y1)));
        let rect = Rect::new(a.left, a.top, b.right, b.bottom);
        canvas.fill_rect_rgba(rect, Rgba::with_alpha(accent.r, accent.g, accent.b, 70));
        canvas.stroke_rect(rect, accent, 2.0);
    }
    let Some(pos) = view.hover else { return };
    match editor.tool {
        Tool::Paint | Tool::Rect | Tool::Fill => {
            let cell = ViewState::cell_of(pos);
            if editor.in_bounds(cell.0, cell.1) {
                canvas.stroke_rect(cell_rect(view, cell), accent, 2.0);
            }
        }
        Tool::AddObject => {
            let ghost = Object {
                id: editor.new_object.0,
                variant: editor.new_object.1,
                x: pos.0 as u16,
                y: pos.1 as u16,
            };
            let rect = paint_object_body(
                canvas,
                editor,
                cache,
                &ghost,
                view.to_screen(pos),
                view.zoom,
            );
            canvas.stroke_rect(rect, object_colour(ghost.id), 1.0);
        }
        Tool::Select => {}
    }
}
