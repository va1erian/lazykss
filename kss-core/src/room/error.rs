//! What can go wrong loading or saving a room.

use super::MAX_CELLS;
use crate::compression::DecompressError;

#[derive(Debug)]
pub enum RoomError {
    BadIndex(usize),
    /// The room pointer or a section pointer leads outside the image.
    BadPointer {
        room: usize,
    },
    Decompress {
        room: usize,
        source: DecompressError,
    },
    /// The blob is shorter than two `width*height` planes.
    ShortMap {
        room: usize,
    },
    /// No free block large enough to move the room to.
    NoFreeSpace {
        needed: usize,
    },
    /// The map does not match `width*height`.
    BadDimensions,
    /// More than [`MAX_CELLS`] cells, or too many objects for one stream.
    TooLarge,
    /// The compressor's output did not inflate back to the input.
    RoundTrip,
}

impl std::fmt::Display for RoomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadIndex(i) => write!(f, "room {i} does not exist"),
            Self::BadPointer { room } => write!(f, "room {room}: pointer outside the ROM"),
            Self::Decompress { room, source } => write!(f, "room {room}: {source}"),
            Self::ShortMap { room } => write!(f, "room {room}: map data is shorter than its size"),
            Self::NoFreeSpace { needed } => write!(f, "no free space for {needed} bytes"),
            Self::BadDimensions => write!(f, "tile map does not match the room size"),
            Self::TooLarge => write!(
                f,
                "the room is too large to store (at most {MAX_CELLS} cells)"
            ),
            Self::RoundTrip => write!(f, "compression self-check failed"),
        }
    }
}

impl std::error::Error for RoomError {}
