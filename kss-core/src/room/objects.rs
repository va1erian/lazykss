//! The object list at the end of the map blob.

/// Size of one object record.
const RECORD: usize = 6;
/// The word that ends the object list.
const TERMINATOR: [u8; 2] = [0xFF, 0xFF];

/// An enemy, item or mechanism placed in a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Object {
    /// Variant: an item's contents, an enemy's behaviour.
    pub variant: u8,
    pub id: u8,
    /// Pixels from the room's left edge.
    pub x: u16,
    /// Pixels from the room's top edge.
    pub y: u16,
}

impl Object {
    /// For a warp star, the index in the room's door list of the record that
    /// says where it goes (a door without a trigger area).
    pub fn warp_door(&self) -> Option<usize> {
        (self.id == crate::names::WARP_STAR).then_some(self.variant as usize)
    }

    /// Reads one record; `None` for the terminator or a short record.
    fn from_record(record: &[u8]) -> Option<Self> {
        let &[variant, id, x_low, x_high, y_low, y_high] = record else {
            return None;
        };
        ([variant, id] != TERMINATOR).then(|| Self {
            variant,
            id,
            x: u16::from_le_bytes([x_low, x_high]),
            y: u16::from_le_bytes([y_low, y_high]),
        })
    }

    /// The record as it is stored in the blob.
    pub(super) fn record(&self) -> [u8; RECORD] {
        let [x_low, x_high] = self.x.to_le_bytes();
        let [y_low, y_high] = self.y.to_le_bytes();
        [self.variant, self.id, x_low, x_high, y_low, y_high]
    }
}

/// Reads the object records that follow the tile planes, and the bytes
/// after their terminator. The list also ends where fewer than a record's
/// bytes are left.
pub(super) fn parse_objects(data: &[u8]) -> (Vec<Object>, Vec<u8>) {
    let objects: Vec<Object> = data
        .chunks_exact(RECORD)
        .map_while(Object::from_record)
        .collect();
    let after = objects
        .len()
        .saturating_mul(RECORD)
        .saturating_add(TERMINATOR.len());
    let tail = data.get(after..).unwrap_or_default().to_vec();
    (objects, tail)
}

/// The object list as it is stored: records, then the terminator.
pub(super) fn encode_objects(objects: &[Object], out: &mut Vec<u8>) {
    out.extend(objects.iter().flat_map(Object::record));
    out.extend_from_slice(&TERMINATOR);
}
