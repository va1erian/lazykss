//! Gestures, undo and redo of the current room.

use super::Editor;
use crate::history::{Change, Entry};

impl Editor {
    /// Starts a gesture: every change until [`end_group`](Self::end_group)
    /// becomes one undo step.
    pub fn begin_group(&mut self) {
        if self.group.is_none() {
            self.group = Some(Vec::new());
        }
    }

    pub fn end_group(&mut self) {
        if let Some(group) = self.group.take()
            && !group.is_empty()
        {
            self.rooms.active_mut().undo.push(group);
        }
    }

    /// Runs `edit` as one undo step, or as part of the gesture already
    /// under way.
    pub(super) fn as_one_step<R>(&mut self, edit: impl FnOnce(&mut Self) -> R) -> R {
        let outer = self.group.is_some();
        self.begin_group();
        let result = edit(self);
        if !outer {
            self.end_group();
        }
        result
    }

    /// Applies `change` to the current room and records it.
    pub(super) fn perform(&mut self, change: Change) {
        let lr = self.rooms.active_mut();
        change.apply(&mut lr.room);
        if change.alters_image() {
            lr.revision += 1;
        }
        lr.dirty = true;
        lr.redo.clear();
        let Some(group) = self.group.as_mut() else {
            lr.undo.push(vec![change]);
            return;
        };
        // Within a gesture, a change folds into the previous one when it
        // can, so a drag does not leave one change per mouse move.
        let unmerged = match group.last_mut() {
            Some(last) => last.merge(change),
            None => Some(change),
        };
        group.extend(unmerged);
    }

    pub fn undo(&mut self) -> bool {
        self.end_group();
        let lr = self.rooms.active_mut();
        let Some(entry) = lr.undo.pop() else {
            return false;
        };
        for change in entry.iter().rev() {
            change.revert(&mut lr.room);
            if change.alters_image() {
                lr.revision += 1;
            }
        }
        lr.dirty = true;
        self.forget_moved_selection(&entry);
        self.rooms.active_mut().redo.push(entry);
        true
    }

    pub fn redo(&mut self) -> bool {
        self.end_group();
        let lr = self.rooms.active_mut();
        let Some(entry) = lr.redo.pop() else {
            return false;
        };
        for change in &entry {
            change.apply(&mut lr.room);
            if change.alters_image() {
                lr.revision += 1;
            }
        }
        lr.dirty = true;
        self.forget_moved_selection(&entry);
        self.rooms.active_mut().undo.push(entry);
        true
    }

    /// After undoing or redoing `entry`, drops a selection whose index may
    /// now name another object or door, or nothing.
    fn forget_moved_selection(&mut self, entry: &Entry) {
        if entry.iter().any(Change::reorders_objects) {
            self.selected_object = None;
        }
        if entry.iter().any(Change::reorders_doors) {
            self.selected_door = None;
        }
        let objects = self.room().objects.len();
        let doors = self.room().doors().len();
        if self.selected_object.is_some_and(|i| i >= objects) {
            self.selected_object = None;
        }
        if self.selected_door.is_some_and(|i| i >= doors) {
            self.selected_door = None;
        }
    }
}
