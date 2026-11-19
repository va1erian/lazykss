//! The xui front end of the editor.

mod app;
mod keys;
mod map;
mod map_paint;
mod picker;
mod render;
mod scroll;
mod sync;
mod tree;
mod view;
mod window;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use xui::prelude::*;

pub use crate::editor::Editor;
pub use app::{DoorField, EditorApp, Layer, Msg, ObjField};

/// The editor, shared between the app (which edits it) and the custom-painted
/// views (which read it when they paint). `None` until a ROM is open.
pub type SharedEditor = Rc<RefCell<Option<Editor>>>;

pub const TITLE: &str = "lazykss \u{2014} Kirby Super Star level editor";

/// Opens the window. `rom` overrides the automatic ROM lookup.
pub fn run(rom: Option<PathBuf>) -> Result<()> {
    xui::app(TITLE).size(WINDOW.0, WINDOW.1).run(move |ui| build_app(ui, rom, new_shared()))
}

/// The window's initial size in design units.
pub const WINDOW: (i32, i32) = (1380, 860);

pub fn new_shared() -> SharedEditor {
    Rc::new(RefCell::new(None))
}

/// Mounts the editor's layout in `ui` and opens the ROM (`rom`, else the
/// usual lookup). `editor` is the shared model the views read; keep a clone
/// of it to inspect the editor from outside (tests do).
pub fn build_app(ui: &mut Ui<Msg>, rom: Option<PathBuf>, editor: SharedEditor) -> Result<EditorApp> {
    window::create(ui, rom, editor)
}
