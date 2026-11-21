//! The HAL encoder: an optimal parse over the longest run of each command
//! at each position.

use std::collections::HashMap;

use super::MAX_RUN;

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
        let operand = if matches!(self.cmd, 1 | 3) { 1 } else { 2 };
        header_len(count).saturating_add(operand)
    }

    /// Input bytes a run of `count` covers.
    fn covers(&self, count: usize) -> usize {
        if self.cmd == 2 {
            count.saturating_mul(2)
        } else {
            count
        }
    }
}

fn header_len(count: usize) -> usize {
    if count > 32 { 2 } else { 1 }
}

/// Writes a chunk header. `count` is 1 to [`MAX_RUN`], as every caller
/// produces it; the casts keep the low bits the format has room for.
fn emit_header(out: &mut Vec<u8>, cmd: u8, count: usize) {
    let n = count.saturating_sub(1);
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

type Positions = HashMap<[u8; 3], Vec<u16>>;

/// Earlier positions, keyed by the three bytes a reference of each kind
/// would start with.
#[derive(Default)]
struct History {
    forward: Positions,
    reversed: Positions,
    backward: Positions,
}

impl History {
    /// Indexes position `p`. Offsets are 16 bits: a later position can never
    /// be the source of a reference, so it is not indexed.
    fn record(&mut self, data: &[u8], p: usize) {
        let Ok(at) = u16::try_from(p) else {
            return;
        };
        if let Some(&key) = data.get(p..).and_then(<[u8]>::first_chunk::<3>) {
            self.forward.entry(key).or_default().push(at);
            let bits = key.map(u8::reverse_bits);
            self.reversed.entry(bits).or_default().push(at);
        }
        let behind = p.checked_sub(2).and_then(|s| data.get(s..=p));
        if let Some(&[c, b, a]) = behind.and_then(<[u8]>::first_chunk::<3>) {
            self.backward.entry([a, b, c]).or_default().push(at);
        }
    }
}

/// The longest match among the most recent `CHAIN` starts in `list`.
fn longest(
    cmd: u8,
    list: Option<&Vec<u16>>,
    limit: usize,
    len_at: impl Fn(usize) -> usize,
) -> Candidate {
    let mut best = Candidate::default();
    for &start in list.into_iter().flatten().rev().take(CHAIN) {
        let len = len_at(usize::from(start));
        if len > usize::from(best.max) {
            best = Candidate {
                cmd,
                max: len as u16,
                arg: start,
            };
        }
        // Nothing is longer than a full-length match.
        if len == limit {
            break;
        }
    }
    best
}

/// Length of the common prefix of two byte sequences.
fn common(a: impl Iterator<Item = u8>, b: &[u8]) -> usize {
    a.zip(b).take_while(|(x, y)| x == *y).count()
}

/// The longest run of each command at `pos`, given the positions before it.
fn find(data: &[u8], pos: usize, history: &History) -> [Candidate; KINDS] {
    let rest = data.get(pos..).unwrap_or_default();
    let Some(&first) = rest.first() else {
        return Default::default();
    };
    let window = rest.get(..MAX_RUN).unwrap_or(rest);
    let limit = window.len();

    let run = window.iter().take_while(|&&b| b == first).count();
    let rle = Candidate {
        cmd: 1,
        max: run as u16,
        arg: first.into(),
    };
    let rle16 = match rest.get(1) {
        Some(&second) => Candidate {
            cmd: 2,
            max: rest
                .chunks_exact(2)
                .take(MAX_RUN)
                .take_while(|c| *c == [first, second])
                .count() as u16,
            arg: u16::from_be_bytes([first, second]),
        },
        None => Candidate::default(),
    };
    let rising = window
        .iter()
        .enumerate()
        .take_while(|&(i, &b)| b == first.wrapping_add(i as u8))
        .count();
    let rising = Candidate {
        cmd: 3,
        max: rising as u16,
        arg: first.into(),
    };

    // A run that already spans the whole window costs two or three bytes:
    // no back-reference beats it, and uniform data is where the search
    // for one is slowest.
    let mut references = [Candidate::default(); 3];
    if let Some(&key) = rest.first_chunk::<3>()
        && run < limit
    {
        // The source of a forward copy may run into the bytes being
        // written: the decompressor copies one byte at a time.
        let from = |start: usize| data.get(start..).unwrap_or_default().iter().copied();
        references = [
            longest(4, history.forward.get(&key), limit, |start| {
                common(from(start), window)
            }),
            longest(5, history.reversed.get(&key), limit, |start| {
                common(from(start).map(u8::reverse_bits), window)
            }),
            longest(6, history.backward.get(&key), limit, |start| {
                let behind = data.get(..=start).unwrap_or_default();
                common(behind.iter().rev().copied(), window)
            }),
        ];
    }
    let [forward, reversed, backward] = references;
    [rle, rle16, rising, forward, reversed, backward]
}

/// Finds, for every position, the longest run of each command.
fn candidates(data: &[u8]) -> Vec<[Candidate; KINDS]> {
    let mut history = History::default();
    (0..data.len())
        .map(|pos| {
            if let Some(p) = pos.checked_sub(1) {
                history.record(data, p);
            }
            find(data, pos, &history)
        })
        .collect()
}

/// One chunk of the parse.
#[derive(Clone, Copy)]
enum Chunk {
    Literal(usize),
    Run(Candidate, usize),
}

/// The cheapest first chunk at a position. `tail[k]` is the cost of the
/// data from `k` bytes on, so `tail.len() - 1` bytes remain.
fn best_chunk(found: &[Candidate; KINDS], tail: &[usize]) -> (usize, Chunk) {
    let remaining = tail.len().saturating_sub(1);
    let mut best = (usize::MAX, Chunk::Literal(0));
    let mut consider = |covered: usize, header: usize, chunk: Chunk| {
        if let Some(&after) = tail.get(covered) {
            let total = header.saturating_add(after);
            if total < best.0 {
                best = (total, chunk);
            }
        }
    };
    for count in 1..=remaining.min(MAX_RUN) {
        let cost = header_len(count).saturating_add(count);
        consider(count, cost, Chunk::Literal(count));
    }
    for candidate in found {
        for count in 1..=usize::from(candidate.max) {
            let chunk = Chunk::Run(*candidate, count);
            consider(candidate.covers(count), candidate.cost(count), chunk);
        }
    }
    best
}

/// Compresses `data` into a stream [`decompress`](super::decompress) (and the game) inflates
/// back to exactly `data`. The parse is optimal for the runs considered.
///
/// Data longer than [`MAX_OUTPUT`](super::MAX_OUTPUT) still encodes, without references past
/// 16-bit offsets, but neither the game nor
/// [`decompress`](super::decompress) accepts it.
pub fn compress(data: &[u8]) -> Vec<u8> {
    let n = data.len();
    let found = candidates(data);

    // cost[i]: fewest bytes that encode data[i..]. choice[i]: the chunk that
    // achieves it.
    let mut cost = vec![0usize; n.saturating_add(1)];
    let mut choice = vec![Chunk::Literal(0); n];
    for (pos, here) in found.iter().enumerate().rev() {
        let (total, chunk) = best_chunk(here, cost.get(pos..).unwrap_or_default());
        if let (Some(c), Some(ch)) = (cost.get_mut(pos), choice.get_mut(pos)) {
            (*c, *ch) = (total, chunk);
        }
    }

    let mut out = Vec::with_capacity(cost.first().map_or(1, |c| c.saturating_add(1)));
    let mut pos = 0;
    while let Some(&chunk) = choice.get(pos) {
        let covered = match chunk {
            Chunk::Literal(count) => {
                let bytes = data.get(pos..).and_then(|r| r.get(..count));
                emit_header(&mut out, 0, count);
                out.extend_from_slice(bytes.unwrap_or_default());
                count
            }
            Chunk::Run(candidate, count) => {
                emit_header(&mut out, candidate.cmd, count);
                match candidate.cmd {
                    1 | 3 => out.push(candidate.arg as u8),
                    _ => out.extend_from_slice(&candidate.arg.to_be_bytes()),
                }
                candidate.covers(count)
            }
        };
        // Every chunk the parse picks covers at least one byte; stopping on
        // an empty one leaves a stream the caller's round trip rejects
        // rather than an endless loop.
        if covered == 0 {
            break;
        }
        pos = pos.saturating_add(covered);
    }
    out.push(0xFF);
    out
}
