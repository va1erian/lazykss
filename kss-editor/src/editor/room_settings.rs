//! The current room's music and size.

use kss_core::room::MAX_CELLS;

use super::Editor;
use crate::history::{Change, MapSnapshot};

impl Editor {
    pub fn set_music(&mut self, song: u8) -> bool {
        let old = self.room().music();
        if old == song {
            return false;
        }
        self.perform(Change::Music { old, new: song });
        true
    }

    /// Resizes the current room, keeping its top-left cells.
    pub fn resize(&mut self, width: usize, height: usize) -> bool {
        let (w, h) = (self.room().width, self.room().height);
        let too_big = width
            .checked_mul(height)
            .is_none_or(|cells| cells > MAX_CELLS);
        if width == 0 || height == 0 || (width, height) == (w, h) || too_big {
            return false;
        }
        let old = MapSnapshot::of(self.room());
        let mut probe = self.room().clone();
        probe.resize(width, height);
        let new = MapSnapshot::of(&probe);
        self.perform(Change::Resize {
            old: Box::new(old),
            new: Box::new(new),
        });
        // Objects outside the new bounds stay; the game ignores them but the
        // user can see and delete them.
        true
    }
}
