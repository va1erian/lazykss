//! Shared helpers for the window tests: build the real window headlessly,
//! inject mouse and keyboard input into the offscreen backend, check the
//! editor model and write screenshots to `target-shots/`. Every test passes
//! silently when no ROM can be found.

// Helpers fail the test the way a test does: by panicking with a message.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Each test file uses its own subset of the helpers.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use kss_core::Rom;
use kss_editor::ui::{Msg, SharedEditor, WINDOW, build_app, new_shared};
use xui::backend::Event;
use xui::prelude::*;
use xui::xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui::{Dip, Theme};

/// Where the map view sits in the 1380x860 window (see the layout report)
/// and its scroll/zoom at start.
pub const MAP: (i32, i32) = (262, 78);
pub const ZOOM: i32 = 2;
/// Tab header positions of the side panel.
pub const TAB_OBJECTS: (i32, i32) = (1106, 88);
pub const TAB_DOORS: (i32, i32) = (1167, 88);
pub const TAB_ROOM: (i32, i32) = (1225, 88);
pub const TAB_TREASURES: (i32, i32) = (1293, 88);

/// The checks that failed during a test step, reported when it ends.
pub type Log = Rc<RefCell<Vec<String>>>;

pub fn check(log: &Log, ok: bool, what: &str) {
    if !ok {
        log.borrow_mut().push(what.to_string());
    }
}

/// Room pixel to window pixel, with the view at its home position (the
/// room's top-left corner at the map's corner, the buffer scrolled away).
pub fn at(px: i32, py: i32) -> (i32, i32) {
    (MAP.0 + px * ZOOM, MAP.1 + py * ZOOM)
}

/// Ctrl and nothing else held.
pub const CTRL: Modifiers = Modifiers {
    ctrl: true,
    ..Modifiers::NONE
};

pub fn mouse_down(s: &Stage<'_, Msg>, (x, y): (i32, i32), button: MouseButton) {
    s.inject(Event::MouseDown {
        x,
        y,
        button,
        modifiers: Modifiers::NONE,
    });
}

pub fn mouse_move(s: &Stage<'_, Msg>, (x, y): (i32, i32)) {
    s.inject(Event::MouseMove {
        x,
        y,
        modifiers: Modifiers::NONE,
    });
}

pub fn mouse_up(s: &Stage<'_, Msg>, (x, y): (i32, i32), button: MouseButton) {
    s.inject(Event::MouseUp {
        x,
        y,
        button,
        modifiers: Modifiers::NONE,
    });
}

pub fn key(s: &Stage<'_, Msg>, key: Key, modifiers: Modifiers) {
    s.inject(Event::KeyDown {
        key,
        modifiers,
        repeat: 1,
        system: false,
    });
}

/// Builds the app, runs `step`, saves the capture as `<name>-light.png`
/// (or `-dark`) and fails if `step` logged a failed check. Returns `false`
/// without doing anything when there is no ROM.
pub fn shoot(
    name: &str,
    theme: Theme,
    step: impl FnOnce(&Stage<'_, Msg>, &SharedEditor, &Log) + 'static,
) -> bool {
    if Rom::locate().is_none() {
        return false;
    }
    shoot_rom(name, theme, None, step)
}

/// Like [`shoot`], opening `rom` instead of the located ROM.
pub fn shoot_rom(
    name: &str,
    theme: Theme,
    rom: Option<std::path::PathBuf>,
    step: impl FnOnce(&Stage<'_, Msg>, &SharedEditor, &Log) + 'static,
) -> bool {
    let shared = new_shared();
    let log: Log = Rc::default();
    let (build_shared, step_shared, step_log) =
        (Rc::clone(&shared), Rc::clone(&shared), Rc::clone(&log));
    let variant = if theme.is_dark { "dark" } else { "light" };
    let image = render_with(
        Snapshot::new(Dip(WINDOW.0 as f32), Dip(WINDOW.1 as f32)).theme(theme),
        move |ui| build_app(ui, rom, build_shared),
        move |stage| step(stage, &step_shared, &step_log),
    )
    .expect("the window renders");
    let dir = std::path::Path::new("target-shots");
    std::fs::create_dir_all(dir).unwrap();
    image
        .save_png(dir.join(format!("{name}-{variant}.png")))
        .unwrap();
    let failures = log.borrow();
    assert!(failures.is_empty(), "failed checks: {failures:?}");
    true
}
