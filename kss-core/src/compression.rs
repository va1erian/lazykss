//! HAL Laboratory LZ codec (the format `exhal`/`inhal` handle).
//!
//! A stream is a sequence of chunks terminated by `0xFF`. Each chunk starts
//! with a command byte `CCCLLLLL` (3-bit command, 5-bit length - 1), or the
//! extended form `111CCCLL LLLLLLLL` (10-bit length - 1).
//!
//! | cmd | meaning |
//! |---|---|
//! | 0 | literal run |
//! | 1 | 8-bit RLE |
//! | 2 | 16-bit RLE (length counts pairs) |
//! | 3 | incrementing run |
//! | 4 | back-reference (absolute big-endian offset into the output) |
//! | 5 | back-reference, each byte bit-reversed |
//! | 6 | back-reference, reading backwards |
//! | 7 | (only reachable through the extended form) same as 4 |

mod encode;

pub use encode::compress;

/// Largest output the game's decompressor can produce.
pub const MAX_OUTPUT: usize = 0x10000;
const MAX_RUN: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecompressError {
    /// The stream ran past the end of the input before a `0xFF` terminator.
    UnexpectedEof,
    /// A back-reference pointed outside the data produced so far.
    BadReference { at: usize },
    /// More than 64 KiB of output.
    TooLarge,
}

impl std::fmt::Display for DecompressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "compressed stream is truncated"),
            Self::BadReference { at } => {
                write!(f, "invalid back-reference at input offset {at:#x}")
            }
            Self::TooLarge => write!(f, "decompressed data exceeds 64 KiB"),
        }
    }
}

impl std::error::Error for DecompressError {}

/// Reads a stream front to back, failing at its end instead of panicking.
struct Reader<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], DecompressError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(DecompressError::UnexpectedEof)?;
        let bytes = self
            .input
            .get(self.pos..end)
            .ok_or(DecompressError::UnexpectedEof)?;
        self.pos = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DecompressError> {
        self.take(N)?
            .try_into()
            .map_err(|_| DecompressError::UnexpectedEof)
    }

    fn byte(&mut self) -> Result<u8, DecompressError> {
        self.array::<1>().map(|[b]| b)
    }
}

/// Decompresses the stream starting at `input[0]`.
///
/// Returns the output and the number of input bytes consumed, terminator
/// included. A chunk that would take the output past [`MAX_OUTPUT`] is
/// refused before anything is written, so a hostile stream cannot make the
/// output grow without bound.
pub fn decompress(input: &[u8]) -> Result<(Vec<u8>, usize), DecompressError> {
    let mut out: Vec<u8> = Vec::new();
    let mut reader = Reader { input, pos: 0 };
    loop {
        let at = reader.pos;
        let control = reader.byte()?;
        if control == 0xFF {
            return Ok((out, reader.pos));
        }
        let (cmd, len) = if control & 0xE0 == 0xE0 {
            let low = reader.byte()?;
            let field = usize::from(control & 3) << 8 | usize::from(low);
            ((control >> 2) & 7, run_length(field))
        } else {
            (control >> 5, run_length(usize::from(control & 0x1F)))
        };
        // 16-bit RLE counts pairs; every other command counts bytes.
        let produced = if cmd == 2 { len.saturating_mul(2) } else { len };
        if out.len().saturating_add(produced) > MAX_OUTPUT {
            return Err(DecompressError::TooLarge);
        }
        match cmd {
            0 => out.extend_from_slice(reader.take(len)?),
            1 => {
                let value = reader.byte()?;
                out.extend(std::iter::repeat_n(value, len));
            }
            2 => {
                let pair = reader.array::<2>()?;
                out.extend(std::iter::repeat_n(pair, len).flatten());
            }
            3 => {
                let first = reader.byte()?;
                // The counter is a byte in the game too: it wraps past FF.
                out.extend((0..len).map(|i| first.wrapping_add(i as u8)));
            }
            _ => {
                let offset = usize::from(u16::from_be_bytes(reader.array::<2>()?));
                copy_reference(&mut out, cmd, offset, len)
                    .ok_or(DecompressError::BadReference { at })?;
            }
        }
    }
}

/// The run length a length field encodes: the field holds length - 1.
fn run_length(field: usize) -> usize {
    field.saturating_add(1)
}

/// Appends `len` bytes copied from `offset` in the output produced so far
/// (commands 4-7). `None` when the source leaves that output.
fn copy_reference(out: &mut Vec<u8>, cmd: u8, offset: usize, len: usize) -> Option<()> {
    for i in 0..len {
        // One byte at a time: a forward source may run into the bytes this
        // very copy is writing, which is how the game repeats a pattern.
        let src = if cmd == 6 {
            offset.checked_sub(i)?
        } else {
            offset.checked_add(i)?
        };
        let value = *out.get(src)?;
        out.push(if cmd == 5 {
            value.reverse_bits()
        } else {
            value
        });
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(data: &[u8]) {
        let packed = compress(data);
        let (unpacked, used) = decompress(&packed).expect("own output decompresses");
        assert_eq!(used, packed.len());
        assert_eq!(unpacked, data);
    }

    #[test]
    fn decodes_each_command() {
        assert_eq!(
            decompress(&[0x02, 0xAA, 0xBB, 0xCC, 0xFF]).unwrap().0,
            [0xAA, 0xBB, 0xCC]
        );
        assert_eq!(decompress(&[0x24, 0xDD, 0xFF]).unwrap().0, [0xDD; 5]);
        assert_eq!(decompress(&[0x41, 1, 2, 0xFF]).unwrap().0, [1, 2, 1, 2]);
        assert_eq!(decompress(&[0x63, 7, 0xFF]).unwrap().0, [7, 8, 9, 10]);
        assert_eq!(
            decompress(&[0x02, 1, 2, 3, 0x82, 0, 0, 0xFF]).unwrap().0,
            [1, 2, 3, 1, 2, 3]
        );
        assert_eq!(
            decompress(&[0x00, 0x01, 0xA0, 0, 0, 0xFF]).unwrap().0,
            [0x01, 0x80]
        );
        assert_eq!(
            decompress(&[0x02, 1, 2, 3, 0xC2, 0, 2, 0xFF]).unwrap().0,
            [1, 2, 3, 3, 2, 1]
        );
        // Extended form: 8-bit RLE of 96 zeroes, as in Green Greens room 1.
        assert_eq!(decompress(&[0xE4, 0x5F, 0x00, 0xFF]).unwrap().0, [0u8; 96]);
    }

    #[test]
    fn reports_malformed_streams() {
        assert_eq!(decompress(&[0x02, 1]), Err(DecompressError::UnexpectedEof));
        assert_eq!(
            decompress(&[0x80, 0, 0, 0xFF]),
            Err(DecompressError::BadReference { at: 0 })
        );
        // Backward copy from before the start of the output.
        assert_eq!(
            decompress(&[0x00, 7, 0xC2, 0, 0, 0xFF]),
            Err(DecompressError::BadReference { at: 2 })
        );
        // Truncated extended header and truncated operands.
        for stream in [&[0xE4][..], &[0x20], &[0x40, 1], &[0x80, 0], &[]] {
            assert_eq!(decompress(stream), Err(DecompressError::UnexpectedEof));
        }
    }

    #[test]
    fn refuses_output_past_the_limit() {
        // 64 maximal RLE chunks reach 64 KiB exactly; one more byte is too much.
        let mut stream = [0xE7, 0xFF, 0x00].repeat(64);
        stream.push(0xFF);
        assert_eq!(decompress(&stream).unwrap().0.len(), MAX_OUTPUT);
        stream.pop();
        stream.extend_from_slice(&[0x20, 0x00, 0xFF]);
        assert_eq!(decompress(&stream), Err(DecompressError::TooLarge));
        // A stream of self-copies cannot grow without bound either.
        let mut copies = vec![0x00, 0x00];
        copies.extend([0xF3, 0xFF, 0x00, 0x00].repeat(1000));
        assert_eq!(decompress(&copies), Err(DecompressError::TooLarge));
    }

    #[test]
    fn arbitrary_input_never_panics() {
        let mut seed = 0x2545_F491u32;
        for _ in 0..2000 {
            let len = (seed % 64) as usize;
            let input: Vec<u8> = (0..len)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (seed >> 24) as u8
                })
                .collect();
            if let Ok((out, used)) = decompress(&input) {
                assert!(out.len() <= MAX_OUTPUT && used <= input.len());
            }
        }
    }

    #[test]
    fn oversized_data_still_encodes() {
        let data: Vec<u8> = (0..MAX_OUTPUT + 300).map(|i| (i % 251) as u8).collect();
        let packed = compress(&data);
        assert_eq!(packed.last(), Some(&0xFF));
        assert_eq!(decompress(&packed), Err(DecompressError::TooLarge));
    }

    #[test]
    fn uniform_data_compresses_quickly() {
        // An empty 64 KiB map: sixty-four long runs, found without searching
        // for back-references at every position.
        let started = std::time::Instant::now();
        let packed = compress(&[0u8; MAX_OUTPUT]);
        assert!(packed.len() <= 64 * 3 + 1, "{} bytes", packed.len());
        assert!(
            started.elapsed().as_secs() < 5,
            "took {:?}",
            started.elapsed()
        );
        round_trip(&[0u8; MAX_OUTPUT]);
    }

    #[test]
    fn round_trips() {
        round_trip(&[]);
        round_trip(&[5]);
        round_trip(&[0; 5000]);
        round_trip(&(0..=255u8).cycle().take(3000).collect::<Vec<_>>());
        let mut mixed = Vec::new();
        let mut seed = 0x1234_5678u32;
        for i in 0..20000u32 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            mixed.push(if i % 97 < 40 {
                (seed >> 24) as u8
            } else {
                (i / 50) as u8
            });
        }
        round_trip(&mixed);
        // Mirrored and bit-reversed repeats exercise commands 5 and 6.
        let base: Vec<u8> = (0..64u8).map(|i| i.wrapping_mul(37) ^ 0x5A).collect();
        let mut tricky = base.clone();
        tricky.extend(base.iter().rev());
        tricky.extend(base.iter().map(|b| b.reverse_bits()));
        let packed = compress(&tricky);
        assert!(
            packed.len() < 100,
            "mirrors should be found: {}",
            packed.len()
        );
        round_trip(&tricky);
    }
}
