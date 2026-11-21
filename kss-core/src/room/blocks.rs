//! The sections of the room header, and section 3: the breakable cells.

use std::num::NonZeroUsize;
use std::ops::Range;

use super::bytes::word;
use super::{Room, SECTION_COUNT, SECTIONS_OFFSET};

/// The section that lists breakable cells.
const BLOCK_SECTION: usize = 3;
/// Size of one block record.
const RECORD: usize = 6;
/// Most records a section can hold: its length is a 16-bit word.
const MAX_BLOCKS: usize = 10_000;
/// The word that ends a section's list.
const TERMINATOR: [u8; 2] = [0xFF, 0xFF];

/// A breakable cell: a record of section 3. A star, bomb or similar block
/// only reacts when its cell is listed here; without a record the metatile
/// is scenery Kirby walks through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub x: usize,
    pub y: usize,
    /// Metatile the cell turns into when the block breaks.
    pub after: u16,
    /// Third word of the record. In the original rooms it is the id of the
    /// cap metatile drawn in the cell above the block, and 0 for the cap's
    /// own record; what the game does with it is not established.
    pub extra: u16,
}

impl Block {
    /// Reads one record of a map `width` cells wide; `None` for the
    /// terminator or a short record.
    fn from_record(record: &[u8], width: usize) -> Option<Self> {
        if record.starts_with(&TERMINATOR) {
            return None;
        }
        // The record holds the cell's byte offset in a tile plane of words.
        let cell = usize::from(word(record, 0)?) / 2;
        // A room without width still lists its blocks, on one column.
        let width = NonZeroUsize::new(width).unwrap_or(NonZeroUsize::MIN);
        Some(Self {
            x: cell % width,
            y: cell / width,
            after: word(record, 2)?,
            extra: word(record, 4)?,
        })
    }

    /// The word that addresses this block's cell in a map `width` cells
    /// wide, when it fits in one.
    fn cell_word(&self, width: usize) -> Option<u16> {
        let cell = self.y.checked_mul(width)?.checked_add(self.x)?;
        u16::try_from(cell.checked_mul(2)?).ok()
    }
}

impl Room {
    /// Range in `head` of section `index`, its length word included. `None`
    /// when the section words lead outside the head.
    fn section_range(&self, index: usize) -> Option<Range<usize>> {
        let mut start = SECTIONS_OFFSET;
        for _ in 0..index {
            start = start.checked_add(usize::from(word(&self.head, start)?))?;
        }
        let end = start.checked_add(usize::from(word(&self.head, start)?))?;
        (end <= self.head.len()).then_some(start..end)
    }

    /// The payload of section `index` (0-5), terminator included. Empty when
    /// the head's section words do not add up.
    pub fn section(&self, index: usize) -> &[u8] {
        let last = SECTION_COUNT.saturating_sub(1);
        self.section_range(index.min(last))
            .and_then(|range| self.head.get(range.start.checked_add(2)?..range.end))
            .unwrap_or_default()
    }

    /// Replaces the payload of section `index`; the sections after it move.
    /// `None`, with the room unchanged, when the head's section words do not
    /// add up or the payload does not fit a section.
    pub(super) fn set_section(&mut self, index: usize, payload: &[u8]) -> Option<()> {
        let range = self.section_range(index)?;
        let len = u16::try_from(payload.len().checked_add(2)?).ok()?;
        let new_end = range.start.checked_add(usize::from(len))?;
        // What lies past the section moves by the change in its length.
        let shift = |at: usize| at.checked_add(new_end)?.checked_sub(range.end);
        let size_at = shift(self.size_at)?;
        let doors_end = if index == 0 {
            shift(self.doors_end)?
        } else {
            self.doors_end
        };
        let section = len.to_le_bytes().into_iter().chain(payload.iter().copied());
        self.head.splice(range, section);
        self.size_at = size_at;
        self.doors_end = doors_end;
        Some(())
    }

    /// The breakable cells of the room.
    pub fn blocks(&self) -> Vec<Block> {
        self.section(BLOCK_SECTION)
            .chunks_exact(RECORD)
            .map_while(|record| Block::from_record(record, self.width))
            .collect()
    }

    /// Replaces the list of breakable cells. Records are stored in map
    /// order, as the game's own rooms have them. Blocks outside the map, or
    /// too far into it for a record to address, are left out. Returns
    /// false, with the room unchanged, when the head's section words do not
    /// add up.
    pub fn set_blocks(&mut self, blocks: &[Block]) -> bool {
        let mut sorted: Vec<(&Block, u16)> = blocks
            .iter()
            .filter(|b| b.x < self.width && b.y < self.height)
            .filter_map(|b| Some((b, b.cell_word(self.width)?)))
            .collect();
        sorted.sort_by_key(|(b, _)| (b.y, b.x));
        sorted.dedup_by_key(|(b, _)| (b.y, b.x));
        sorted.truncate(MAX_BLOCKS);
        let mut payload: Vec<u8> = sorted
            .iter()
            .flat_map(|(block, cell)| [*cell, block.after, block.extra])
            .flat_map(u16::to_le_bytes)
            .collect();
        payload.extend_from_slice(&TERMINATOR);
        self.set_section(BLOCK_SECTION, &payload).is_some()
    }

    /// Brings the list of breakable cells in line with the map.
    ///
    /// `before(x, y)` is the tile the cell held when the room was last
    /// loaded or saved (`None` outside that map). A cell that now holds a
    /// `breakable` metatile and did not then gets a record: the one in
    /// `remembered` for that cell when there is one (so a block that was
    /// painted away and put back keeps what it breaks into), else one that
    /// breaks to the empty tile. A record is dropped when its cell was a
    /// block then and no longer is. Other records are left alone: the game's
    /// rooms also list decoration cells, and leave a few blocks unlisted.
    ///
    /// Returns true when the list changed.
    pub fn sync_blocks(
        &mut self,
        breakable: impl Fn(u16) -> bool,
        before: impl Fn(usize, usize) -> Option<u16>,
        remembered: &[Block],
    ) -> bool {
        let old = self.blocks();
        let mut blocks: Vec<Block> = old
            .iter()
            .copied()
            .filter(|b| b.x < self.width && b.y < self.height)
            .filter(|b| {
                !(before(b.x, b.y).is_some_and(&breakable) && !breakable(self.tile(b.x, b.y)))
            })
            .collect();
        for y in 0..self.height {
            for x in 0..self.width {
                let tile = self.tile(x, y);
                let placed = before(x, y) != Some(tile);
                if placed && breakable(tile) && !blocks.iter().any(|b| (b.x, b.y) == (x, y)) {
                    let known = remembered.iter().find(|b| (b.x, b.y) == (x, y));
                    blocks.push(known.copied().unwrap_or(Block {
                        x,
                        y,
                        after: 0,
                        extra: 0,
                    }));
                }
            }
        }
        blocks.sort_by_key(|b| (b.y, b.x));
        blocks != old && self.set_blocks(&blocks)
    }
}
