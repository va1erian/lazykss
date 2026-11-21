//! Painting the map: the buffer around the room, the room image, grid,
//! doors, objects and tool overlays.

use kss_core::names::{TREASURE_CHEST, WARP_STAR};
use kss_core::{CELL, Object};
use xui::Color;
use xui::Dip;
use xui::backend::{Canvas, ImageSampling, Rgba, TextStyle};
use xui::geometry::{Point, Rect};
use xui::theme::Theme;

use crate::editor::{Cell, Editor, Tool};

use super::render::RoomImages;
use super::view::ViewState;

const TEXT: Dip = Dip(10.0);

/// Paints the map into `canvas` (the node's own canvas).
pub fn paint(
    canvas: &mut dyn Canvas,
    editor: &Option<Editor>,
    view: &mut ViewState,
    cache: &mut RoomImages,
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

/// What the cached room image was rendered from: the opened ROM, the room,
/// its edit revision and the layers shown.
fn cache_key(editor: &Editor, view: &ViewState) -> u64 {
    let layers = view.layers.background as u64 | (view.layers.foreground as u64) << 1;
    (editor.session() & 0xFFF) << 52
        | (editor.current_index() as u64 & 0xFFF) << 40
        | (editor.revision() & 0x3F_FFFF_FFFF) << 2
        | layers
}

fn paint_room(
    canvas: &mut dyn Canvas,
    editor: &Editor,
    view: &mut ViewState,
    cache: &mut RoomImages,
    theme: &Theme,
) {
    let room = editor.room();
    view.set_room(room.width, room.height);

    cache.set_session(editor.session());
    let key = cache_key(editor, view);
    if cache.key != key {
        cache.set_room(key, &editor.tileset().render_room(room, view.layers));
    }

    let area = view.scroller.view();
    let zoom = view.zoom;
    canvas.push_clip(area);

    paint_buffer_backdrop(canvas, editor, view, area, theme);
    // The canvas scales the image with nearest-neighbour sampling and
    // paints only what the clip shows.
    if let Some(image) = cache.room() {
        let (left, top) = view.to_screen((0, 0));
        let (w, h) = (image.width() as i32 * zoom, image.height() as i32 * zoom);
        canvas.draw_image_sampled(
            image,
            Rect::new(left, top, left + w, top + h),
            ImageSampling::Nearest,
        );
    }
    if view.layers.foreground {
        paint_buffer_tiles(canvas, editor, view, cache, area);
    }
    if view.grid {
        paint_grid(canvas, editor, view, area);
    }
    // The room's edge, where the saved map ends and the buffer begins.
    let edge = cells_rect(view, (0, 0, room.width as i32, room.height as i32));
    canvas.stroke_rect(
        Rect::new(edge.left - 1, edge.top - 1, edge.right + 1, edge.bottom + 1),
        theme.accent,
        2.0,
    );
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

/// The cells of the map and buffer that `area` shows:
/// `(left, top, right, bottom)`, right and bottom exclusive.
fn visible_cells(editor: &Editor, view: &ViewState, area: Rect) -> (i32, i32, i32, i32) {
    let (l, t, r, b) = editor.canvas_cells();
    let first = ViewState::cell_of(view.to_room(area.left, area.top));
    let last = ViewState::cell_of(view.to_room(area.right - 1, area.bottom - 1));
    (
        l.max(first.0),
        t.max(first.1),
        r.min(last.0 + 1),
        b.min(last.1 + 1),
    )
}

/// The screen rectangle of a range of cells, right and bottom exclusive.
fn cells_rect(view: &ViewState, (l, t, r, b): (i32, i32, i32, i32)) -> Rect {
    let c = CELL as i32;
    let a = view.to_screen((l * c, t * c));
    let z = view.to_screen((r * c, b * c));
    Rect::new(a.0, a.1, z.0, z.1)
}

/// A checkerboard under the buffer cells, so the scratch area never reads as
/// part of the room.
fn paint_buffer_backdrop(
    canvas: &mut dyn Canvas,
    editor: &Editor,
    view: &ViewState,
    area: Rect,
    theme: &Theme,
) {
    let a = theme.accent;
    let tones = [
        Rgba::with_alpha(a.r, a.g, a.b, 20),
        Rgba::with_alpha(a.r, a.g, a.b, 50),
    ];
    let (l, t, r, b) = visible_cells(editor, view, area);
    for y in t..b {
        for x in l..r {
            if !editor.in_bounds(x, y) {
                canvas.fill_rect_rgba(cell_rect(view, (x, y)), tones[((x + y) & 1) as usize]);
            }
        }
    }
}

/// One metatile over the room's backdrop colour, as the game would show it.
fn paint_metatile(
    canvas: &mut dyn Canvas,
    editor: &Editor,
    view: &ViewState,
    cache: &mut RoomImages,
    cell: (i32, i32),
    id: u16,
) {
    let rect = cell_rect(view, cell);
    let [r, g, b, _] = editor.tileset().backdrop();
    canvas.fill_rect_rgba(rect, Rgba::with_alpha(r, g, b, 255));
    let key = (editor.session(), editor.current_index());
    if let Some(image) = cache.metatile(key, editor.tileset(), id) {
        canvas.draw_image_sampled(image, rect, ImageSampling::Nearest);
    }
}

fn paint_buffer_tiles(
    canvas: &mut dyn Canvas,
    editor: &Editor,
    view: &ViewState,
    cache: &mut RoomImages,
    area: Rect,
) {
    let (l, t, r, b) = visible_cells(editor, view, area);
    for (&(x, y), &id) in editor.buffer() {
        if x >= l && x < r && y >= t && y < b && !editor.in_bounds(x, y) {
            paint_metatile(canvas, editor, view, cache, (x, y), id);
        }
    }
}

fn paint_grid(canvas: &mut dyn Canvas, editor: &Editor, view: &ViewState, area: Rect) {
    let line = Rgba::with_alpha(255, 255, 255, 70);
    let c = CELL as i32;
    let (l, t, r, b) = visible_cells(editor, view, area);
    let (left, top) = view.to_screen((l * c, t * c));
    let (right, bottom) = view.to_screen((r * c, b * c));
    for i in l..=r {
        let x = view.to_screen((i * c, 0)).0;
        canvas.fill_rect_rgba(Rect::new(x, top, x + 1, bottom), line);
    }
    for j in t..=b {
        let y = view.to_screen((0, j * c)).1;
        canvas.fill_rect_rgba(Rect::new(left, y, right, y + 1), line);
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
        // The position is the top-left corner of the door's cell.
        let (left, top) = view.to_screen((door.x as i32, door.y as i32));
        let rect = Rect::new(left, top, left + side, top + side);
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
            &format!("\u{2192} {}", editor.destination_label(door)),
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
    cache: &mut RoomImages,
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
    let icon = cache.sprite(editor.sprites(), object.id, object.variant);
    if let (Some(icon), Some((ox, oy))) = (icon, origin) {
        let (left, top) = (cx - ox * zoom, cy - oy * zoom);
        let (w, h) = (icon.width() as i32 * zoom, icon.height() as i32 * zoom);
        let rect = Rect::new(left, top, left + w, top + h);
        canvas.draw_image_sampled(icon, rect, ImageSampling::Nearest);
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
    cache: &mut RoomImages,
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
        // A sprite speaks for itself; a box, or the selection, gets its
        // name. Chests and warp stars always say what they hold or reach.
        let has_sprite = editor.sprites().get(object.id, object.variant).is_some();
        let telling = matches!(object.id, TREASURE_CHEST | WARP_STAR);
        if selected || !has_sprite || telling {
            let name = editor.object_label(object);
            let name = if name.is_empty() {
                format!("object {:02X}", object.id)
            } else {
                name
            };
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
    cache: &mut RoomImages,
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
            if editor.cell(cell.0, cell.1) != Cell::Outside {
                canvas.stroke_rect(cell_rect(view, cell), accent, 2.0);
            }
        }
        Tool::Stamp => {
            let cell = ViewState::cell_of(pos);
            match &editor.stamp {
                // The stamp, ghosted where a click would paste it.
                Some(stamp) if view.rect_drag.is_none() => {
                    for dy in 0..stamp.height {
                        for dx in 0..stamp.width {
                            if let Some(id) = stamp.get(dx, dy) {
                                let at = (cell.0 + dx, cell.1 + dy);
                                paint_metatile(canvas, editor, view, cache, at, id);
                            }
                        }
                    }
                    let rect = cells_rect(
                        view,
                        (cell.0, cell.1, cell.0 + stamp.width, cell.1 + stamp.height),
                    );
                    canvas.fill_rect_rgba(rect, Rgba::with_alpha(accent.r, accent.g, accent.b, 60));
                    canvas.stroke_rect(rect, accent, 2.0);
                }
                _ => {
                    if editor.cell(cell.0, cell.1) != Cell::Outside {
                        canvas.stroke_rect(cell_rect(view, cell), accent, 2.0);
                    }
                }
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
