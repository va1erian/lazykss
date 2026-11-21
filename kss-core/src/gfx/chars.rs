//! SNES 4bpp characters and little-endian tilemap words.

/// Bytes of one 4bpp character.
pub(crate) const CHAR_BYTES: usize = 32;

/// Little-endian words; a trailing odd byte is dropped.
pub(crate) fn le_words(bytes: &[u8]) -> impl Iterator<Item = u16> + '_ {
    bytes
        .chunks_exact(2)
        .filter_map(|w| w.try_into().ok())
        .map(u16::from_le_bytes)
}

/// An 8x8 character as palette indices (0-15), rows top to bottom.
pub(crate) type Char = [[u8; 8]; 8];

/// Decodes a 4bpp character: bit planes 0 and 1 interleaved by row in the
/// first 16 bytes, planes 2 and 3 in the next 16. `None` when fewer than
/// 32 bytes are given.
pub(crate) fn decode_char(bytes: &[u8]) -> Option<Char> {
    let (low, high) = bytes.get(..CHAR_BYTES)?.split_at_checked(16)?;
    let mut char = [[0u8; 8]; 8];
    for ((row, lo), hi) in char
        .iter_mut()
        .zip(low.chunks_exact(2))
        .zip(high.chunks_exact(2))
    {
        // The leftmost pixel is the most significant bit of each plane.
        for (pixel, shift) in row.iter_mut().zip((0..8).rev()) {
            *pixel = lo
                .iter()
                .chain(hi)
                .enumerate()
                .fold(0, |acc, (bit, plane)| acc | ((plane >> shift) & 1) << bit);
        }
    }
    Some(char)
}

/// The pixels of a character as `(x, y, palette index)`, flipped as asked.
pub(crate) fn char_pixels(
    char: &Char,
    hflip: bool,
    vflip: bool,
) -> impl Iterator<Item = (usize, usize, u8)> + '_ {
    // `i ^ 7` mirrors an index within 0..8.
    let mirror = |i: usize, flip: bool| if flip { i ^ 7 } else { i };
    char.iter().enumerate().flat_map(move |(y, row)| {
        row.iter()
            .enumerate()
            .map(move |(x, &value)| (mirror(x, hflip), mirror(y, vflip), value))
    })
}
