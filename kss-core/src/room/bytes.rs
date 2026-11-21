//! Bounds-checked reads of the little-endian values rooms are made of.

/// Base of the SA-1's linear mapping of the image (banks `C0-FF`).
const SNES_BASE: usize = 0xC0_0000;

/// `base + index * stride`, when it does not overflow.
pub(super) fn offset_of(base: usize, index: usize, stride: usize) -> Option<usize> {
    base.checked_add(index.checked_mul(stride)?)
}

/// The little-endian word at `at`, when both bytes are there.
pub(super) fn word(data: &[u8], at: usize) -> Option<u16> {
    let bytes = data.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes(bytes.try_into().ok()?))
}

/// The 3-byte SNES long pointer at `at`, as a file offset into `data`.
/// `None` when it is cut short or does not point into the image.
pub(super) fn long_pointer(data: &[u8], at: usize) -> Option<usize> {
    let &[low, high, bank] = data.get(at..at.checked_add(3)?)? else {
        return None;
    };
    let snes = u32::from_le_bytes([low, high, bank, 0]) as usize;
    snes.checked_sub(SNES_BASE).filter(|&pc| pc < data.len())
}
