//! `jam` and `cue`: Urbit's bit-level noun serialization.
//!
//! The encoding is a little-endian bitstream. Every noun starts with a tag:
//! `0` atom, `1 0` cell (then head, then tail), `1 1` backreference to the
//! bit offset where an equal noun was first encoded. Atoms and backreference
//! offsets are length-prefixed with `mat`.
//!
//! [`jam`] reproduces Hoon's `++jam` exactly (cells are always backreferenced
//! when repeated; a repeated atom is re-encoded when it is no longer than the
//! offset), so the output is byte-identical to what a ship produces.

use std::collections::HashMap;
use std::fmt;

use crate::noun::{Arena, Noun, View, met0, trim};

/// Serialize a noun. The result is the little-endian bytes of the jam atom
/// (no trailing zero bytes), i.e. the contents of a `.jam` file.
pub fn jam(arena: &Arena, root: Noun) -> Vec<u8> {
    jam_counting(arena, root).0
}

/// [`jam`], also returning how many nouns it visited (the work it did).
pub fn jam_counting(arena: &Arena, root: Noun) -> (Vec<u8>, u64) {
    let mut out = BitWriter::default();
    let mut seen: HashMap<Noun, u64> = HashMap::new();
    let mut stack = vec![root];
    let mut visited = 0u64;
    while let Some(noun) = stack.pop() {
        visited += 1;
        let pos = out.len;
        if let Some(&back) = seen.get(&noun) {
            match arena.view(noun) {
                View::Atom(bytes) if met0(bytes) <= 64 - u64::from(back.leading_zeros()) => {
                    out.bit(false);
                    out.mat(bytes);
                }
                _ => {
                    out.bit(true);
                    out.bit(true);
                    out.mat(&back.to_le_bytes());
                }
            }
            continue;
        }
        seen.insert(noun, pos);
        match arena.view(noun) {
            View::Atom(bytes) => {
                out.bit(false);
                out.mat(bytes);
            }
            View::Cell(head, tail) => {
                out.bit(true);
                out.bit(false);
                stack.push(tail);
                stack.push(head);
            }
        }
    }
    (out.into_bytes(), visited)
}

/// Why a byte string could not be cued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CueError {
    /// The input ended in the middle of a noun.
    Truncated { bit: u64 },
    /// An atom length or backreference does not fit in 64 bits.
    TooLarge { bit: u64 },
    /// A backreference names an offset where no complete noun starts.
    BadBackref { bit: u64, target: u64 },
}

impl fmt::Display for CueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CueError::Truncated { bit } => write!(f, "jam data is truncated (at bit {bit})"),
            CueError::TooLarge { bit } => {
                write!(
                    f,
                    "jam data has an impossibly large length prefix (at bit {bit})"
                )
            }
            CueError::BadBackref { bit, target } => write!(
                f,
                "jam data has a backreference to bit {target}, where no noun starts (at bit {bit})"
            ),
        }
    }
}

impl std::error::Error for CueError {}

/// Result of [`cue`]: the noun plus how much of the input it occupied.
pub struct Cued {
    pub noun: Noun,
    /// Bits consumed by the encoding.
    pub bits_read: u64,
    /// Whether any set bits follow the encoding (Hoon ignores them).
    pub trailing_garbage: bool,
}

/// Most nouns [`cue`] will decode. A noun can be encoded in four bits, so
/// without a bound a small hostile file could demand gigabytes of memory;
/// real deskjams decode to a few thousand nouns per megabyte.
pub const MAX_NOUNS: usize = 1 << 24;

/// Deserialize a noun from the little-endian bytes of a jam atom.
///
/// Hostile input is fine: everything is bounds-checked, nothing recurses,
/// and memory use is bounded by the input size and [`MAX_NOUNS`]
/// (backreferences share structure instead of copying it).
pub fn cue(arena: &mut Arena, bytes: &[u8]) -> Result<Cued, CueError> {
    cue_with_limit(arena, bytes, MAX_NOUNS)
}

/// [`cue`], decoding at most `max_nouns` nouns.
pub fn cue_with_limit(arena: &mut Arena, bytes: &[u8], max_nouns: usize) -> Result<Cued, CueError> {
    enum Frame {
        /// Decoding the head of the cell whose memo slot is given.
        Head(usize),
        Tail(usize, Noun),
    }
    let mut input = BitReader::new(bytes);
    // Every noun decoded so far, by the bit offset where it starts. Nouns
    // start in increasing order, so this stays sorted; a cell's slot is
    // filled in once both halves are done.
    let mut memo: Vec<(u64, Option<Noun>)> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    loop {
        if memo.len() >= max_nouns {
            return Err(CueError::TooLarge { bit: input.pos });
        }
        let pos = input.pos;
        let mut done = if !input.bit()? {
            let atom = input.rub()?;
            let noun = arena.atom(&atom);
            memo.push((pos, Some(noun)));
            noun
        } else if !input.bit()? {
            memo.push((pos, None));
            stack.push(Frame::Head(memo.len() - 1));
            continue;
        } else {
            let target = input.rub()?;
            let target = trim(&target);
            if target.len() > 8 {
                return Err(CueError::BadBackref {
                    bit: pos,
                    target: u64::MAX,
                });
            }
            let mut buf = [0u8; 8];
            buf[..target.len()].copy_from_slice(target);
            let target = u64::from_le_bytes(buf);
            memo.binary_search_by_key(&target, |&(at, _)| at)
                .ok()
                .and_then(|slot| memo[slot].1)
                .ok_or(CueError::BadBackref { bit: pos, target })?
        };
        loop {
            match stack.pop() {
                None => {
                    let trailing_garbage = input.any_set_after();
                    return Ok(Cued {
                        noun: done,
                        bits_read: input.pos,
                        trailing_garbage,
                    });
                }
                Some(Frame::Head(slot)) => {
                    stack.push(Frame::Tail(slot, done));
                    break;
                }
                Some(Frame::Tail(slot, head)) => {
                    done = arena.cell(head, done);
                    memo[slot].1 = Some(done);
                }
            }
        }
    }
}

#[derive(Default)]
struct BitWriter {
    buf: Vec<u8>,
    /// Number of bits written; `buf` always holds exactly `ceil(len / 8)` bytes.
    len: u64,
}

impl BitWriter {
    fn bit(&mut self, bit: bool) {
        let shift = self.len % 8;
        if shift == 0 {
            self.buf.push(0);
        }
        if bit {
            *self.buf.last_mut().unwrap() |= 1 << shift;
        }
        self.len += 1;
    }

    /// Write the low `count` bits of `value`, least significant first.
    fn bits(&mut self, value: u64, count: u32) {
        for i in 0..count {
            self.bit((value >> i) & 1 == 1);
        }
    }

    /// Write the `met0(atom)` significant bits of an atom.
    fn atom_bits(&mut self, atom: &[u8]) {
        let atom = trim(atom);
        let width = met0(atom);
        if width == 0 {
            return;
        }
        let shift = (self.len % 8) as u32;
        if shift == 0 {
            self.buf.extend_from_slice(atom);
        } else {
            let mut carry = u16::from(self.buf.pop().unwrap());
            for &byte in atom {
                let merged = carry | (u16::from(byte) << shift);
                self.buf.push(merged as u8);
                carry = merged >> 8;
            }
            if self.buf.len() < (self.len + width).div_ceil(8) as usize {
                self.buf.push(carry as u8);
            }
        }
        self.len += width;
        debug_assert_eq!(self.buf.len() as u64, self.len.div_ceil(8));
    }

    /// Hoon's `mat`: self-delimiting atom encoding.
    fn mat(&mut self, atom: &[u8]) {
        let width = met0(atom);
        if width == 0 {
            self.bit(true);
            return;
        }
        let width_width = 64 - width.leading_zeros();
        self.bits(0, width_width);
        self.bit(true);
        self.bits(width, width_width - 1);
        self.atom_bits(atom);
    }

    fn into_bytes(self) -> Vec<u8> {
        let mut buf = self.buf;
        let len = trim(&buf).len();
        buf.truncate(len);
        buf
    }
}

struct BitReader<'a> {
    buf: &'a [u8],
    pos: u64,
    end: u64,
}

impl<'a> BitReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        BitReader {
            buf,
            pos: 0,
            end: buf.len() as u64 * 8,
        }
    }

    fn bit(&mut self) -> Result<bool, CueError> {
        if self.pos >= self.end {
            return Err(CueError::Truncated { bit: self.pos });
        }
        let bit = (self.buf[(self.pos / 8) as usize] >> (self.pos % 8)) & 1 == 1;
        self.pos += 1;
        Ok(bit)
    }

    /// Read `count <= 64` bits as a little-endian integer. Like Hoon's
    /// `cut`, bits past the end of the input read as zero.
    fn bits(&mut self, count: u32) -> Result<u64, CueError> {
        let mut value = 0u64;
        for i in 0..count {
            if self.pos < self.end && (self.buf[(self.pos / 8) as usize] >> (self.pos % 8)) & 1 == 1
            {
                value |= 1 << i;
            }
            self.pos += 1;
        }
        Ok(value)
    }

    /// Read `width` bits as a little-endian byte string. Bits past the end
    /// of the input read as zero (and are not allocated).
    fn atom(&mut self, width: u64) -> Result<Vec<u8>, CueError> {
        let start_bit = self.pos;
        let after = start_bit
            .checked_add(width)
            .ok_or(CueError::TooLarge { bit: start_bit })?;
        let present = width.min(self.end.saturating_sub(start_bit));
        let mut out = Vec::with_capacity(present.div_ceil(8) as usize);
        if present > 0 {
            let start = (start_bit / 8) as usize;
            let shift = (start_bit % 8) as u32;
            let len = present.div_ceil(8) as usize;
            for i in 0..len {
                let lo = self.buf[start + i] >> shift;
                let hi = match self.buf.get(start + i + 1) {
                    Some(&next) if shift > 0 => next << (8 - shift),
                    _ => 0,
                };
                out.push(lo | hi);
            }
            let spare = (len as u64 * 8 - present) as u32;
            if spare > 0 {
                *out.last_mut().unwrap() &= 0xff >> spare;
            }
        }
        self.pos = after;
        Ok(out)
    }

    /// Hoon's `rub`: decode a `mat`-encoded atom.
    fn rub(&mut self) -> Result<Vec<u8>, CueError> {
        let start = self.pos;
        let mut zeros = 0u32;
        while !self.bit()? {
            zeros += 1;
            // vere's cue gives up at 64 (its lengths are under 2^63 bits).
            if zeros >= 64 {
                return Err(CueError::TooLarge { bit: start });
            }
        }
        if zeros == 0 {
            return Ok(Vec::new());
        }
        let width = (1u64 << (zeros - 1)) | self.bits(zeros - 1)?;
        self.atom(width)
    }

    fn any_set_after(&self) -> bool {
        if self.pos >= self.end {
            return false;
        }
        let byte = (self.pos / 8) as usize;
        let shift = (self.pos % 8) as u32;
        (self.buf[byte] >> shift) != 0 || self.buf[byte + 1..].iter().any(|&b| b != 0)
    }
}
