//! Parsing a room block out of the image.

use super::bytes::word;
use super::objects::parse_objects;
use super::{Room, RoomError, SECTION_COUNT, SECTIONS_OFFSET, Tables};
use crate::compression;
use crate::rom::Rom;

impl Room {
    pub fn load(rom: &Rom, tables: &Tables, index: usize) -> Result<Self, RoomError> {
        let offset = *tables
            .room_offsets
            .get(index)
            .ok_or(RoomError::BadIndex(index))?;
        let data = rom.data();
        let bad = || RoomError::BadPointer { room: index };
        let size_word = find_size_word(data, offset).ok_or_else(bad)?;
        let width = usize::from(word(data, size_word).ok_or_else(bad)?);
        let height_at = size_word.checked_add(2).ok_or_else(bad)?;
        let height = usize::from(word(data, height_at).ok_or_else(bad)?);
        let blob_at = size_word.checked_add(4).ok_or_else(bad)?;
        let packed = data.get(blob_at..).ok_or_else(bad)?;
        let (blob, packed_len) =
            compression::decompress(packed).map_err(|source| RoomError::Decompress {
                room: index,
                source,
            })?;
        let (tiles, rest) = width
            .checked_mul(height)
            .and_then(|cells| split_planes(&blob, cells))
            .ok_or(RoomError::ShortMap { room: index })?;
        let (objects, tail) = parse_objects(rest);
        let head = data.get(offset..blob_at).ok_or_else(bad)?.to_vec();
        // `find_size_word` only walks forward from `offset`.
        let size_at = size_word.checked_sub(offset).ok_or_else(bad)?;
        let stored_len = head.len().checked_add(packed_len).ok_or_else(bad)?;
        // Section 0 is the door list: the door slots end where it does.
        let doors_end = word(&head, SECTIONS_OFFSET)
            .and_then(|len| SECTIONS_OFFSET.checked_add(usize::from(len)))
            .ok_or_else(bad)?;
        Ok(Self {
            index,
            offset,
            stored_len,
            head,
            size_at,
            doors_end: doors_end.min(size_at),
            width,
            height,
            tiles,
            objects,
            tail,
        })
    }
}

/// File offset of the width word of the room block at `offset`: past the
/// six sections, each of which starts with its own length, and the `3DEF`
/// word. `None` when a section has a length under 2 (it would not move
/// forward) or the walk leaves the image before the width and height.
fn find_size_word(data: &[u8], offset: usize) -> Option<usize> {
    let mut section = offset.checked_add(SECTIONS_OFFSET)?;
    for _ in 0..SECTION_COUNT {
        let step = usize::from(word(data, section)?);
        if step < 2 {
            return None;
        }
        section = section.checked_add(step)?;
    }
    let size_word = section.checked_add(2)?;
    // Width and height must both be in the image.
    (size_word.checked_add(4)? <= data.len()).then_some(size_word)
}

/// Splits the blob into its two tile planes, combined into metatile ids,
/// and what follows them. `None` when the blob is shorter than the planes.
fn split_planes(blob: &[u8], cells: usize) -> Option<(Vec<u16>, &[u8])> {
    let (low, rest) = blob.split_at_checked(cells)?;
    let (high, rest) = rest.split_at_checked(cells)?;
    let tiles = low
        .iter()
        .zip(high)
        .map(|(&low, &high)| u16::from_le_bytes([low, high]))
        .collect();
    Some((tiles, rest))
}
