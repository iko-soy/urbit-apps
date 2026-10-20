//! Hoon's `mug` (31-bit murmur3-based noun hash) and the `gor`/`mor`
//! orderings derived from it, which determine the shape of Hoon maps and sets.

use std::cmp::Ordering;

use crate::noun::{Arena, Noun, View, trim};

/// MurmurHash3 x86 32-bit (Hoon's `muk`, vere's `MurmurHash3_x86_32`).
pub fn murmur3_32(data: &[u8], seed: u32) -> u32 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;
    let mut h = seed;
    let (blocks, tail) = data.as_chunks::<4>();
    for block in blocks {
        let mut k = u32::from_le_bytes(*block);
        k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        h ^= k;
        h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }
    if !tail.is_empty() {
        let mut k = 0u32;
        for (i, &byte) in tail.iter().enumerate() {
            k |= (byte as u32) << (8 * i);
        }
        k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        h ^= k;
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h
}

/// Hoon's `mum`: fold murmur3 to 31 bits, retrying with the next seed while
/// the result is zero, and fall back to `fal` after eight attempts.
fn mum(seed: u32, fal: u32, key: &[u8]) -> u32 {
    for i in 0..8 {
        let haz = murmur3_32(key, seed.wrapping_add(i));
        let ham = (haz >> 31) ^ (haz & 0x7fff_ffff);
        if ham != 0 {
            return ham;
        }
    }
    fal
}

/// `mug` of an atom given its little-endian bytes.
pub fn mug_atom(bytes: &[u8]) -> u32 {
    mum(0xcafe_babe, 0x7fff, trim(bytes))
}

/// `mug` of a cell given the mugs of its head and tail:
/// `(mum 0xdead.beef 0xfffe (cat 5 head tail))`.
pub fn mug_cell(head: u32, tail: u32) -> u32 {
    let both = (head as u64) | ((tail as u64) << 32);
    mum(0xdead_beef, 0xfffe, trim(&both.to_le_bytes()))
}

impl Arena {
    /// Hoon's `mug`, memoized per node.
    pub fn mug(&self, noun: Noun) -> u32 {
        let cached = self.mugs[noun.index()].get();
        if cached != 0 {
            return cached;
        }
        let mut stack = vec![noun];
        while let Some(&top) = stack.last() {
            if self.mugs[top.index()].get() != 0 {
                stack.pop();
                continue;
            }
            let mug = match self.view(top) {
                View::Atom(bytes) => mug_atom(bytes),
                View::Cell(h, t) => {
                    let (mh, mt) = (self.mugs[h.index()].get(), self.mugs[t.index()].get());
                    if mh == 0 || mt == 0 {
                        if mh == 0 {
                            stack.push(h);
                        }
                        if mt == 0 {
                            stack.push(t);
                        }
                        continue;
                    }
                    mug_cell(mh, mt)
                }
            };
            self.mugs[top.index()].set(mug);
            stack.pop();
        }
        self.mugs[noun.index()].get()
    }

    /// `mug` of the atom whose value is `(mug noun)`; the priority used by `mor`.
    fn double_mug(&self, noun: Noun) -> u32 {
        mug_atom(&self.mug(noun).to_le_bytes())
    }

    /// Hoon's `gor`: order by mug, ties broken by `dor`. This is the key order
    /// of map and set treaps (the left subtree holds `gor`-smaller keys).
    pub fn gor(&self, a: Noun, b: Noun) -> bool {
        let (c, d) = (self.mug(a), self.mug(b));
        if c == d { self.dor(a, b) } else { c < d }
    }

    /// Hoon's `mor`: order by double mug, ties broken by `dor`. This is the
    /// heap order of treaps (a node is `mor`-before its children).
    pub fn mor(&self, a: Noun, b: Noun) -> bool {
        let (c, d) = (self.double_mug(a), self.double_mug(b));
        if c == d { self.dor(a, b) } else { c < d }
    }

    /// `gor` as a total order, for sorting.
    pub fn gor_cmp(&self, a: Noun, b: Noun) -> Ordering {
        if a == b {
            Ordering::Equal
        } else if self.gor(a, b) {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }
}
