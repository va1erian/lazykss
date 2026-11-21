//! The file and edit commands: open, save, quit, undo and redo.

use std::path::PathBuf;

use xui::prelude::*;

use super::{EditorApp, Gesture, Msg};
use crate::editor::{Editor, default_edited_path};

impl EditorApp {
    pub(super) fn open_rom(&mut self, path: PathBuf) {
        match Editor::open(&path) {
            Ok(editor) => {
                *self.editor.borrow_mut() = Some(editor);
                self.expanded = super::tree::initial_expansion();
                self.dirty_seen.clear();
                self.tree_rows.clear();
                self.room_changed();
                self.say(&format!("Opened {}", path.display()));
            }
            Err(e) => self.say(&format!("Could not open ROM: {e}")),
        }
    }

    pub(super) fn save(&mut self, as_path: Option<PathBuf>) {
        let result = self.ed_mut(|e| match &as_path {
            Some(path) => e.save_as(path),
            None => e.save(),
        });
        match result {
            Some(Ok(summary)) => {
                self.say(&summary.describe());
                self.refresh_tree();
            }
            Some(Err(e)) => self.say(&format!("Save failed: {e}")),
            None => self.say("Nothing to save: no ROM is open"),
        }
    }

    /// Whether unsaved edits stop `action` this time. The first request only
    /// warns; asking again straight away goes ahead (see `update`).
    fn unsaved_edits_block(&mut self, action: &str) -> bool {
        if self.ed(|e| e.any_dirty()).unwrap_or(false) && !self.discard_armed {
            self.discard_armed = true;
            self.say(&format!(
                "Unsaved changes. Save first, or choose {action} again to discard them."
            ));
            return true;
        }
        false
    }

    /// Open ROM: asks for the file, but first warns once about unsaved edits.
    pub(super) fn request_open(&mut self) {
        if self.unsaved_edits_block("Open ROM") {
            return;
        }
        self.discard_armed = false;
        if let Some(dir) = self
            .ed(|e| e.source_path().parent().map(|p| p.to_path_buf()))
            .flatten()
        {
            self.open_dialog.set_initial_dir(dir);
        }
        self.open_dialog.open();
    }

    /// Save As: asks for the file, suggesting the name Save would use.
    pub(super) fn request_save_as(&mut self) {
        let Some((dir, name)) = self.ed(|e| {
            let target = default_edited_path(e.source_path());
            (
                target.parent().map(|p| p.to_path_buf()),
                target.file_name().map(|n| n.to_string_lossy().into_owned()),
            )
        }) else {
            return;
        };
        if let Some(dir) = dir {
            self.save_dialog.set_initial_dir(dir);
        }
        if let Some(name) = name {
            self.save_dialog.set_suggested_name(&name);
        }
        self.save_dialog.open();
    }

    pub(super) fn quit(&mut self, ui: &mut Ui<Msg>) {
        if !self.unsaved_edits_block("Quit") {
            ui.quit();
        }
    }

    pub(super) fn undo_redo(&mut self, redo: bool) {
        self.gesture = Gesture::Idle;
        let done = self
            .ed_mut(|e| if redo { e.redo() } else { e.undo() })
            .unwrap_or(false);
        if done {
            self.after_edit();
            self.say(if redo { "Redo" } else { "Undo" });
        }
    }
}
