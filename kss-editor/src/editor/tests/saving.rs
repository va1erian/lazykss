//! Saving: where Save writes, what reaches the file, and what a failed
//! write leaves behind.

use std::path::{Path, PathBuf};

use kss_core::Rom;

use super::{editor, temp};
use crate::editor::{Editor, default_edited_path};

#[test]
fn default_path_is_next_to_source_and_differs() {
    let p = default_edited_path(Path::new("C:/roms/Kirby Super Star (USA).sfc"));
    assert_eq!(
        p,
        PathBuf::from("C:/roms/Kirby Super Star (USA) (edited).sfc")
    );
    // An existing file is never the default target.
    let dir = temp("default-path");
    let source = dir.join("game.sfc");
    assert_eq!(default_edited_path(&source), dir.join("game (edited).sfc"));
    std::fs::write(dir.join("game (edited).sfc"), b"earlier session").unwrap();
    assert_eq!(
        default_edited_path(&source),
        dir.join("game (edited 2).sfc")
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn edits_survive_a_save_and_the_original_is_untouched() {
    let Some(path) = Rom::locate() else { return };
    let original = std::fs::read(&path).unwrap();
    let dir = temp("save");
    // Work on a copy so the test cannot touch the real ROM even by accident.
    let source = dir.join("Kirby Super Star (USA).sfc");
    std::fs::write(&source, &original).unwrap();

    let mut e = Editor::open(&source).unwrap();
    e.tile = 0x0C;
    let old_tile = e.room().tile(3, 2);
    assert_ne!(old_tile, 0x0C, "pick a cell that actually changes");
    assert!(e.set_tile(3, 2, 0x0C));
    e.new_object = (0x1A, 1);
    let added = e.add_object(250, 77);
    let first = e.room().objects[0];
    e.move_object(0, first.x as i32 + 2, first.y as i32 + 3);
    let expected_objects = e.room().objects.clone();

    // Plain Save defaults to a new file next to the source.
    let summary = e.save().unwrap();
    assert_eq!(
        summary.path,
        dir.join("Kirby Super Star (USA) (edited).sfc")
    );
    assert_ne!(summary.path, source);
    assert_eq!(summary.rooms.len(), 1);
    assert!(!e.any_dirty());
    assert_eq!(
        std::fs::read(&source).unwrap(),
        original,
        "the source ROM must be untouched"
    );

    let reloaded = Editor::open(&summary.path).unwrap();
    assert_eq!(reloaded.room().tile(3, 2), 0x0C);
    assert_eq!(reloaded.room().objects, expected_objects);
    assert_eq!(reloaded.room().objects[added].id, 0x1A);

    // After the first save, Save keeps writing to the edited file.
    assert_eq!(e.save_target(), summary.path);
    e.set_tile(4, 2, 0x0C);
    let again = e.save().unwrap();
    assert_eq!(again.path, summary.path);
    assert_eq!(std::fs::read(&source).unwrap(), original);

    // Save As the source is explicit and allowed.
    e.save_as(&source).unwrap();
    assert_ne!(std::fs::read(&source).unwrap(), original);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn resized_room_round_trips() {
    let Some(path) = Rom::locate() else { return };
    let dir = temp("resize");
    let source = dir.join("rom.sfc");
    std::fs::copy(&path, &source).unwrap();
    let mut e = Editor::open(&source).unwrap();
    e.tile = 0x0C;
    e.resize(104, 8);
    e.set_tile(103, 7, 0x0C);
    let summary = e.save().unwrap();
    let r = Editor::open(&summary.path).unwrap();
    assert_eq!((r.room().width, r.room().height), (104, 8));
    assert_eq!(r.room().tile(103, 7), 0x0C);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_failed_write_keeps_the_rooms_dirty() {
    let Some(mut e) = editor() else { return };
    e.tile = 0x00C;
    e.paint(40, 1);
    let missing = temp("failed-write").join("no-such-folder").join("out.sfc");
    assert!(e.save_as(&missing).is_err());
    assert!(e.any_dirty(), "the edit is not on disk");
    // The next save writes it.
    let good = temp("failed-write").join("out.sfc");
    let summary = e.save_as(&good).unwrap();
    assert_eq!(summary.rooms.len(), 1);
    assert!(!e.any_dirty());
    let reopened = Editor::open(&good).unwrap();
    assert_eq!(reopened.tile_at(40, 1), Some(0x00C));
    let _ = std::fs::remove_dir_all(temp("failed-write"));
}

#[test]
fn resizing_keeps_breakable_blocks_on_their_cells() {
    let Some(mut e) = editor() else { return };
    // Room 3: star blocks at (19..24, 4), their caps in the row above.
    e.select_room(3).unwrap();
    let blocks = e.room().blocks();
    assert_eq!(blocks.len(), 8);
    assert!(e.resize(40, 8));
    assert_eq!(e.room().blocks(), blocks);
    let out = temp("resize-blocks").join("out.sfc");
    e.save_as(&out).unwrap();
    let mut reopened = Editor::open(&out).unwrap();
    reopened.select_room(3).unwrap();
    assert_eq!((reopened.room().width, reopened.room().height), (40, 8));
    assert_eq!(reopened.room().blocks(), blocks);
    // Undo restores the size and the list.
    assert!(e.undo());
    assert_eq!((e.room().width, e.room().blocks()), (37, blocks));
    // A room that could not be stored is refused.
    assert!(!e.resize(400, 100));
    let _ = std::fs::remove_dir_all(temp("resize-blocks"));
}

#[test]
fn a_block_restored_by_undo_after_a_save_keeps_its_record() {
    let Some(mut e) = editor() else { return };
    e.select_room(3).unwrap();
    let blocks = e.room().blocks();
    let dir = temp("undo-block");
    e.tile = 0;
    e.paint(19, 4);
    e.save_as(&dir.join("a.sfc")).unwrap();
    assert_eq!(e.room().blocks().len(), 7);
    assert!(e.undo());
    e.save_as(&dir.join("b.sfc")).unwrap();
    assert_eq!(e.room().blocks(), blocks);
    let _ = std::fs::remove_dir_all(dir);
}
