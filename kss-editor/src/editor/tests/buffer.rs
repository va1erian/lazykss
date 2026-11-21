//! The scratch buffer around the map and the stamp tool.

use super::super::{BUFFER_MARGIN, Cell};
use super::editor;

#[test]
fn the_buffer_is_undoable_but_never_modifies_the_room() {
    let Some(mut e) = editor() else { return };
    let (w, h) = (e.room().width as i32, e.room().height as i32);
    let before = e.room().tiles.clone();
    assert_eq!(e.cell(-1, 0), Cell::Buffer(None));
    assert_eq!(e.cell(w + BUFFER_MARGIN, 0), Cell::Outside);
    assert_eq!(e.cell(0, h + BUFFER_MARGIN - 1), Cell::Buffer(None));
    // Painting, filling and erasing below the room touch only the buffer.
    e.tile = 0x0C;
    assert!(e.paint(2, h + 1));
    assert!(!e.paint(2, h + BUFFER_MARGIN), "beyond the buffer");
    assert_eq!(e.fill_rect((0, h + 3), (1, h + 4)), 4);
    assert!(e.erase(0, h + 3));
    assert!(!e.erase(0, 0), "map cells cannot be emptied");
    assert_eq!(e.buffer().len(), 4);
    assert_eq!(e.room().tiles, before);
    assert!(!e.is_dirty(e.current_index()), "the buffer is not saved");
    e.undo();
    assert_eq!(e.cell(0, h + 3), Cell::Buffer(Some(0x0C)));
    e.undo();
    e.undo();
    assert!(e.buffer().is_empty());
    e.redo();
    assert_eq!(e.cell(2, h + 1), Cell::Buffer(Some(0x0C)));
    assert!(!e.is_dirty(e.current_index()));
    // A rectangle across the edge fills both, as separate cells.
    e.tile = 0x0D;
    assert_eq!(e.fill_rect((-1, 0), (0, 0)), 2);
    assert!(e.is_dirty(e.current_index()));
    e.undo();
    assert_eq!(e.room().tiles, before);
    // A flood fill in the buffer stays in the buffer.
    let n = e.flood_fill(-3, -3);
    assert!(n > 100, "the empty buffer is one region");
    assert_eq!(e.room().tiles, before);
    assert_eq!(
        e.cell(-BUFFER_MARGIN, -BUFFER_MARGIN),
        Cell::Buffer(Some(0x0D))
    );
    e.undo();
    assert_eq!(e.buffer().len(), 1);
}

#[test]
fn a_stamp_copies_from_the_buffer_and_pastes_on_the_map() {
    let Some(mut e) = editor() else { return };
    let h = e.room().height as i32;
    // Compose an L in the buffer: two cells, a hole at (1, h + 2).
    e.tile = 0x0C;
    e.paint(0, h + 1);
    e.tile = 0x0D;
    e.paint(0, h + 2);
    e.paint(1, h + 1);
    assert_eq!(e.copy_stamp((3, h + 5), (5, h + 6)), None, "empty cells");
    assert_eq!(e.copy_stamp((1, h + 2), (0, h + 1)), Some((2, 2)));
    let hole = e.room().tile(11, 3);
    e.begin_group();
    assert!(e.paste_stamp((10, 2)) > 0);
    e.end_group();
    assert_eq!(e.room().tile(10, 2), 0x0C);
    assert_eq!(e.room().tile(10, 3), 0x0D);
    assert_eq!(e.room().tile(11, 2), 0x0D);
    assert_eq!(e.room().tile(11, 3), hole, "empty stamp cells keep the map");
    assert!(e.is_dirty(e.current_index()));
    e.undo();
    assert_eq!(
        e.room().tile(10, 2),
        e.loaded(e.current_index()).unwrap().room.tile(10, 2)
    );
    // Map cells copy too, and a paste into the buffer stays there.
    let tile = e.room().tile(20, 1);
    e.copy_stamp((20, 1), (20, 1));
    e.paste_stamp((-2, -2));
    assert_eq!(e.cell(-2, -2), Cell::Buffer(Some(tile)));
}
