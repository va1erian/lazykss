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

use std::collections::HashMap;

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

/// Decompresses the stream starting at `input[0]`.
///
/// Returns the output and the number of input bytes consumed, terminator
/// included.
pub fn decompress(input: &[u8]) -> Result<(Vec<u8>, usize), DecompressError> {
    let mut out: Vec<u8> = Vec::new();
    let mut pos = 0usize;
    let byte = |pos: usize| {
        input
            .get(pos)
            .copied()
            .ok_or(DecompressError::UnexpectedEof)
    };
    loop {
        let at = pos;
        let control = byte(pos)?;
        pos += 1;
        if control == 0xFF {
            return Ok((out, pos));
        }
        let (cmd, len) = if control & 0xE0 == 0xE0 {
            let len = ((control as usize & 3) << 8 | byte(pos)? as usize) + 1;
            pos += 1;
            ((control >> 2) & 7, len)
        } else {
            (control >> 5, (control as usize & 0x1F) + 1)
        };
        match cmd {
            0 => {
                let run = input
                    .get(pos..pos + len)
                    .ok_or(DecompressError::UnexpectedEof)?;
                out.extend_from_slice(run);
                pos += len;
            }
            1 => {
                let value = byte(pos)?;
                pos += 1;
                out.resize(out.len() + len, value);
            }
            2 => {
                let (a, b) = (byte(pos)?, byte(pos + 1)?);
                pos += 2;
                for _ in 0..len {
                    out.push(a);
                    out.push(b);
                }
            }
            3 => {
                let value = byte(pos)?;
                pos += 1;
                for i in 0..len {
                    out.push(value.wrapping_add(i as u8));
                }
            }
            _ => {
                let offset = (byte(pos)? as usize) << 8 | byte(pos + 1)? as usize;
                pos += 2;
                for i in 0..len {
                    let src = if cmd == 6 {
                        offset.checked_sub(i)
                    } else {
                        Some(offset + i)
                    };
                    let value = src
                        .and_then(|s| out.get(s).copied())
                        .ok_or(DecompressError::BadReference { at })?;
                    out.push(if cmd == 5 {
                        value.reverse_bits()
                    } else {
                        value
                    });
                }
            }
        }
        if out.len() > MAX_OUTPUT {
            return Err(DecompressError::TooLarge);
        }
    }
}

/// The longest run of one command at one position. Any shorter run of the
/// same command is valid too.
#[derive(Clone, Copy, Default)]
struct Candidate {
    cmd: u8,
    /// Longest usable value of the length field + 1 (pairs for 16-bit RLE).
    max: u16,
    /// Operand: fill byte(s) or back-reference offset.
    arg: u16,
}

impl Candidate {
    /// Encoded size of a run of `count`.
    fn cost(&self, count: usize) -> usize {
        header_len(count) + if matches!(self.cmd, 1 | 3) { 1 } else { 2 }
    }

    /// Input bytes a run of `count` covers.
    fn covers(&self, count: usize) -> usize {
        if self.cmd == 2 { count * 2 } else { count }
    }
}

fn header_len(count: usize) -> usize {
    if count > 32 { 2 } else { 1 }
}

fn emit_header(out: &mut Vec<u8>, cmd: u8, count: usize) {
    debug_assert!((1..=MAX_RUN).contains(&count));
    let n = count - 1;
    if n < 32 {
        out.push(cmd << 5 | n as u8);
    } else {
        out.push(0xE0 | cmd << 2 | (n >> 8) as u8);
        out.push(n as u8);
    }
}

/// Number of earlier occurrences tried per back-reference kind.
const CHAIN: usize = 128;
const KINDS: usize = 6;

/// Finds, for every position, the longest run of each command.
fn candidates(data: &[u8]) -> Vec<[Candidate; KINDS]> {
    let n = data.len();
    // Earlier positions, keyed by the three bytes a reference of each kind
    // would start with.
    let mut forward: HashMap<[u8; 3], Vec<u32>> = HashMap::new();
    let mut reversed: HashMap<[u8; 3], Vec<u32>> = HashMap::new();
    let mut backward: HashMap<[u8; 3], Vec<u32>> = HashMap::new();
    let mut all = Vec::with_capacity(n);
    for pos in 0..n {
        if pos > 0 {
            let p = pos - 1;
            if p + 2 < n {
                forward
                    .entry([data[p], data[p + 1], data[p + 2]])
                    .or_default()
                    .push(p as u32);
                let bits = [
                    data[p].reverse_bits(),
                    data[p + 1].reverse_bits(),
                    data[p + 2].reverse_bits(),
                ];
                reversed.entry(bits).or_default().push(p as u32);
            }
            if p >= 2 {
                backward
                    .entry([data[p], data[p - 1], data[p - 2]])
                    .or_default()
                    .push(p as u32);
            }
        }
        let rest = &data[pos..];
        let limit = rest.len().min(MAX_RUN);
        let mut found = [Candidate::default(); KINDS];

        let run = rest[..limit].iter().take_while(|&&b| b == rest[0]).count();
        found[0] = Candidate {
            cmd: 1,
            max: run as u16,
            arg: rest[0] as u16,
        };
        if rest.len() >= 2 {
            let pairs = rest
                .chunks_exact(2)
                .take(MAX_RUN)
                .take_while(|c| c[0] == rest[0] && c[1] == rest[1])
                .count();
            found[1] = Candidate {
                cmd: 2,
                max: pairs as u16,
                arg: (rest[0] as u16) << 8 | rest[1] as u16,
            };
        }
        let rising = rest[..limit]
            .iter()
            .enumerate()
            .take_while(|&(i, &b)| b == rest[0].wrapping_add(i as u8))
            .count();
        found[2] = Candidate {
            cmd: 3,
            max: rising as u16,
            arg: rest[0] as u16,
        };

        // A run that already spans the whole window costs two or three bytes:
        // no back-reference beats it, and uniform data is where the search
        // for one is slowest.
        if rest.len() >= 3 && run < limit {
            let key = [rest[0], rest[1], rest[2]];
            let mut longest =
                |slot: usize, cmd: u8, list: Option<&Vec<u32>>, len_at: &dyn Fn(usize) -> usize| {
                    for &start in list.into_iter().flatten().rev().take(CHAIN) {
                        let len = len_at(start as usize);
                        if len > found[slot].max as usize {
                            found[slot] = Candidate {
                                cmd,
                                max: len as u16,
                                arg: start as u16,
                            };
                        }
                        // Nothing is longer than a full-length match.
                        if len == limit {
                            break;
                        }
                    }
                };
            // The source of a forward copy may run into the bytes being
            // written: the decompressor copies one byte at a time.
            longest(3, 4, forward.get(&key), &|start| {
                (0..limit)
                    .take_while(|&i| data[start + i] == rest[i])
                    .count()
            });
            longest(4, 5, reversed.get(&key), &|start| {
                (0..limit)
                    .take_while(|&i| data[start + i].reverse_bits() == rest[i])
                    .count()
            });
            longest(5, 6, backward.get(&key), &|start| {
                (0..limit.min(start + 1))
                    .take_while(|&i| data[start - i] == rest[i])
                    .count()
            });
        }
        all.push(found);
    }
    all
}

/// Compresses `data` into a stream [`decompress`] (and the game) inflates
/// back to exactly `data`. The parse is optimal for the runs considered.
///
/// # Panics
///
/// Panics if `data` is longer than [`MAX_OUTPUT`]: back-reference offsets are
/// 16 bits.
pub fn compress(data: &[u8]) -> Vec<u8> {
    assert!(data.len() <= MAX_OUTPUT, "HAL streams hold at most 64 KiB");
    let n = data.len();
    let found = candidates(data);

    // cost[i]: fewest bytes that encode data[i..]. choice[i]: the chunk that
    // achieves it, as (kind, count); kind KINDS is a literal run.
    let mut cost = vec![0usize; n + 1];
    let mut choice = vec![(KINDS as u8, 0u16); n + 1];
    for pos in (0..n).rev() {
        let mut best = (usize::MAX, KINDS as u8, 0u16);
        for count in 1..=(n - pos).min(MAX_RUN) {
            let total = header_len(count) + count + cost[pos + count];
            if total < best.0 {
                best = (total, KINDS as u8, count as u16);
            }
        }
        for (kind, candidate) in found[pos].iter().enumerate() {
            for count in 1..=candidate.max as usize {
                let total = candidate.cost(count) + cost[pos + candidate.covers(count)];
                if total < best.0 {
                    best = (total, kind as u8, count as u16);
                }
            }
        }
        cost[pos] = best.0;
        choice[pos] = (best.1, best.2);
    }

    let mut out = Vec::with_capacity(cost[0] + 1);
    let mut pos = 0;
    while pos < n {
        let (kind, count) = (choice[pos].0 as usize, choice[pos].1 as usize);
        if kind == KINDS {
            emit_header(&mut out, 0, count);
            out.extend_from_slice(&data[pos..pos + count]);
            pos += count;
        } else {
            let candidate = found[pos][kind];
            emit_header(&mut out, candidate.cmd, count);
            match candidate.cmd {
                1 | 3 => out.push(candidate.arg as u8),
                _ => out.extend_from_slice(&candidate.arg.to_be_bytes()),
            }
            pos += candidate.covers(count);
        }
    }
    out.push(0xFF);
    out
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
