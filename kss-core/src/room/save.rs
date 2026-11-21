//! Encoding a room and writing it back into the image.

use super::bytes::offset_of;
use super::objects::encode_objects;
use super::{MAX_CELLS, Room, RoomError, Tables};
use crate::compression;
use crate::rom::Rom;

/// Bytes per entry of the room pointer table.
const ROOM_POINTER: usize = 4;
/// The image is padded bank by bank.
const BANK: usize = 0x1_0000;
/// Free runs shorter than this are not worth tracking.
const MIN_FREE_RUN: usize = 0x200;
/// Bytes skipped at the start of a run of `FF`: the first ones can be the
/// terminators of the data that precedes it.
const FREE_GUARD: usize = 16;

/// What [`Room::save`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveReport {
    pub offset: usize,
    pub len: usize,
    pub previous_len: usize,
    /// True when the room no longer fitted and was moved to free space.
    pub moved: bool,
}

impl Room {
    /// The uncompressed map blob: two tile planes, then the object list.
    pub fn blob(&self) -> Vec<u8> {
        let planes = self.tiles.iter().map(|tile| tile.to_le_bytes());
        let mut blob: Vec<u8> = planes.clone().map(|[low, _]| low).collect();
        blob.extend(planes.map(|[_, high]| high));
        encode_objects(&self.objects, &mut blob);
        blob.extend_from_slice(&self.tail);
        blob
    }

    /// The whole room block as it is written to the ROM.
    pub fn encode(&self) -> Result<Vec<u8>, RoomError> {
        let cells = self.width.checked_mul(self.height);
        let width = u16::try_from(self.width).map_err(|_| RoomError::BadDimensions)?;
        let height = u16::try_from(self.height).map_err(|_| RoomError::BadDimensions)?;
        if cells != Some(self.tiles.len()) {
            return Err(RoomError::BadDimensions);
        }
        let blob = self.blob();
        if self.tiles.len() > MAX_CELLS || blob.len() > compression::MAX_OUTPUT {
            return Err(RoomError::TooLarge);
        }
        let packed = compression::compress(&blob);
        match compression::decompress(&packed) {
            Ok((back, used)) if back == blob && used == packed.len() => {}
            _ => return Err(RoomError::RoundTrip),
        }
        let mut block = self.head.clone();
        let size = self
            .size_at
            .checked_add(4)
            .and_then(|end| block.get_mut(self.size_at..end))
            .ok_or(RoomError::BadPointer { room: self.index })?;
        for (dst, src) in size
            .iter_mut()
            .zip(width.to_le_bytes().into_iter().chain(height.to_le_bytes()))
        {
            *dst = src;
        }
        block.extend_from_slice(&packed);
        Ok(block)
    }

    /// Writes the room into `rom`.
    ///
    /// The block is written over the original when it fits. Otherwise it
    /// moves to free space, the room pointer follows it, and the old block
    /// is released. `self` and `tables` are updated to the new location.
    pub fn save(&mut self, rom: &mut Rom, tables: &mut Tables) -> Result<SaveReport, RoomError> {
        let block = self.encode()?;
        let previous_len = self.stored_len;
        let end = self.offset.checked_add(self.stored_len);
        if end.is_none_or(|end| end > rom.data().len()) {
            return Err(RoomError::BadPointer { room: self.index });
        }
        let moved = block.len() > self.stored_len;
        if moved {
            self.offset = self.move_to_free_space(rom, tables, &block)?;
        } else {
            // Over the original, padded with FF to its old length.
            let mut padded = block.clone();
            padded.resize(self.stored_len, 0xFF);
            if !rom.write(self.offset, &padded) {
                return Err(RoomError::BadPointer { room: self.index });
            }
        }
        self.stored_len = block.len();
        rom.fix_checksum();
        Ok(SaveReport {
            offset: self.offset,
            len: block.len(),
            previous_len,
            moved,
        })
    }

    /// Writes `block` to free space, points the room table at it, and
    /// releases the room's old block unless another room shares it. Returns
    /// the new offset.
    fn move_to_free_space(
        &self,
        rom: &mut Rom,
        tables: &mut Tables,
        block: &[u8],
    ) -> Result<usize, RoomError> {
        let pointer = offset_of(tables.room_table, self.index, ROOM_POINTER)
            .filter(|&at| at.checked_add(3).is_some_and(|end| end <= rom.data().len()))
            .ok_or(RoomError::BadIndex(self.index))?;
        if self.index >= tables.room_offsets.len() {
            return Err(RoomError::BadIndex(self.index));
        }
        // The old block is free too, but data is never moved onto itself:
        // a failed write must leave the original intact.
        let target = find_free(rom.data(), block.len()).ok_or(RoomError::NoFreeSpace {
            needed: block.len(),
        })?;
        let bad = RoomError::BadPointer { room: self.index };
        if !rom.write(target, block) || !rom.write_long_pointer(pointer, target) {
            return Err(bad);
        }
        let shared = tables
            .room_offsets
            .iter()
            .enumerate()
            .any(|(i, &offset)| i != self.index && offset == self.offset);
        if !shared {
            // Checked against the image at the start of `save`.
            if !rom.write(self.offset, &vec![0xFF; self.stored_len]) {
                return Err(bad);
            }
        }
        if let Some(offset) = tables.room_offsets.get_mut(self.index) {
            *offset = target;
        }
        Ok(target)
    }
}

/// The run of `FF` that pads the end of each bank, as `(start, len)`, guard
/// bytes excluded. Only this padding is treated as free: a run of `FF` inside
/// a bank can be data (blank graphics, tables).
fn free_runs(data: &[u8]) -> impl Iterator<Item = (usize, usize)> + '_ {
    data.chunks(BANK).enumerate().filter_map(|(bank, bytes)| {
        let used = bytes
            .iter()
            .rposition(|&b| b != 0xFF)
            .map_or(0, |last| last.saturating_add(1));
        let run = bytes.len().checked_sub(used)?;
        if run < MIN_FREE_RUN {
            return None;
        }
        let start = offset_of(used, bank, BANK)?.checked_add(FREE_GUARD)?;
        Some((start, run.checked_sub(FREE_GUARD)?))
    })
}

/// Finds `needed` free bytes, in the smallest bank padding that fits.
fn find_free(data: &[u8], needed: usize) -> Option<usize> {
    free_runs(data)
        .filter(|&(_, len)| len >= needed)
        .min_by_key(|&(_, len)| len)
        .map(|(start, _)| start)
}

/// Total free bytes [`Room::save`] could move rooms into.
pub fn free_space(rom: &Rom) -> usize {
    free_runs(rom.data())
        .map(|(_, len)| len)
        .fold(0, usize::saturating_add)
}
