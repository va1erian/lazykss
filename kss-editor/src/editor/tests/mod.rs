//! Tests of the editing model. Those that need the ROM pass silently when
//! `Rom::locate` cannot find it.

mod buffer;
mod doors;
mod editing;
mod saving;

use std::path::PathBuf;

use kss_core::Rom;

use super::{Editor, line_cells};

fn editor() -> Option<Editor> {
    let path = Rom::locate()?;
    Some(Editor::open(&path).expect("ROM opens"))
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kss-editor-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn lines_have_no_gaps() {
    let cells = line_cells((0, 0), (5, 2));
    assert_eq!(cells.first(), Some(&(0, 0)));
    assert_eq!(cells.last(), Some(&(5, 2)));
    for pair in cells.windows(2) {
        assert!((pair[1].0 - pair[0].0).abs() <= 1 && (pair[1].1 - pair[0].1).abs() <= 1);
    }
    assert_eq!(line_cells((3, 3), (3, 3)), vec![(3, 3)]);
    assert_eq!(line_cells((2, 0), (0, 0)), vec![(2, 0), (1, 0), (0, 0)]);
}

#[test]
fn opens_green_greens_room_one() {
    let Some(e) = editor() else { return };
    assert_eq!(e.current_index(), 1);
    assert_eq!((e.room().width, e.room().height), (100, 8));
    let named = e.named_rooms();
    assert_eq!(named.len(), 27 + 81);
    assert_eq!(e.cave_rooms(), 72..153);
    assert!(e.loaded(26).is_some() && e.loaded(152).is_some());
    assert!(e.loaded(27).is_none());
    assert!(!e.any_dirty());
}

#[test]
fn each_opened_rom_has_its_own_session() {
    let (Some(a), Some(b)) = (editor(), editor()) else {
        return;
    };
    assert_ne!(a.session(), b.session());
    assert_eq!(
        (a.current_index(), a.revision()),
        (b.current_index(), b.revision())
    );
}
