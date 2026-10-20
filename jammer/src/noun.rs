//! Nouns stored in a hash-consed arena.
//!
//! Every structurally distinct noun is stored exactly once, so two [`Noun`]
//! handles from the same [`Arena`] are equal iff the nouns they denote are
//! equal. That makes jam's backreference table, Hoon's `dor` ordering and
//! duplicate detection O(1) per node, and lets every traversal be iterative:
//! desk files decode to lists with hundreds of thousands of elements, so
//! nothing here may recurse on noun depth.

use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::rc::Rc;

/// Handle to a noun inside an [`Arena`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Noun(u32);

/// The atom `0` (`~`), which every arena stores first.
pub const NULL: Noun = Noun(0);

impl Noun {
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }
}

/// Borrowed view of a noun.
#[derive(Copy, Clone, Debug)]
pub enum View<'a> {
    /// Little-endian bytes of the atom, without trailing zero bytes
    /// (so the atom `0` is the empty slice).
    Atom(&'a [u8]),
    Cell(Noun, Noun),
}

enum Node {
    Atom(Rc<[u8]>),
    Cell(Noun, Noun),
}

/// Storage for nouns. Nouns are never freed individually; drop the arena.
pub struct Arena {
    nodes: Vec<Node>,
    /// Memoized `mug` per node, 0 = not computed yet (a mug is never 0).
    pub(crate) mugs: Vec<Cell<u32>>,
    atoms: HashMap<Rc<[u8]>, Noun>,
    cells: HashMap<(Noun, Noun), Noun>,
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

impl Arena {
    pub fn new() -> Self {
        let mut arena = Arena {
            nodes: Vec::new(),
            mugs: Vec::new(),
            atoms: HashMap::new(),
            cells: HashMap::new(),
        };
        // `~` is everywhere; give it a fixed handle.
        let null: Rc<[u8]> = Rc::from(&[][..]);
        let n = arena.push(Node::Atom(null.clone()));
        arena.atoms.insert(null, n);
        arena
    }

    /// Number of distinct nouns stored.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn push(&mut self, node: Node) -> Noun {
        let id = u32::try_from(self.nodes.len()).expect("noun arena exhausted");
        self.nodes.push(node);
        self.mugs.push(Cell::new(0));
        Noun(id)
    }

    /// The atom whose little-endian bytes are `bytes` (trailing zero bytes
    /// are insignificant, as in `as-octs`/`met`).
    pub fn atom(&mut self, bytes: &[u8]) -> Noun {
        let bytes = trim(bytes);
        if bytes.is_empty() {
            return NULL;
        }
        if let Some(&n) = self.atoms.get(bytes) {
            return n;
        }
        let rc: Rc<[u8]> = Rc::from(bytes);
        let n = self.push(Node::Atom(rc.clone()));
        self.atoms.insert(rc, n);
        n
    }

    pub fn atom_u64(&mut self, value: u64) -> Noun {
        self.atom(&value.to_le_bytes())
    }

    /// A cord (`@t`): the UTF-8 bytes of `text` as a little-endian atom.
    pub fn cord(&mut self, text: &str) -> Noun {
        self.atom(text.as_bytes())
    }

    pub fn cell(&mut self, head: Noun, tail: Noun) -> Noun {
        let next = self.nodes.len();
        match self.cells.entry((head, tail)) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let n = Noun(u32::try_from(next).expect("noun arena exhausted"));
                entry.insert(n);
                self.nodes.push(Node::Cell(head, tail));
                self.mugs.push(Cell::new(0));
                n
            }
        }
    }

    /// A null-terminated list `~[a b c]`.
    pub fn list(&mut self, items: &[Noun]) -> Noun {
        let mut list = self.atom(&[]);
        for &item in items.iter().rev() {
            list = self.cell(item, list);
        }
        list
    }

    /// A path (`(list @ta)`) from its segments.
    pub fn path<S: AsRef<str>>(&mut self, segments: &[S]) -> Noun {
        let items: Vec<Noun> = segments.iter().map(|s| self.cord(s.as_ref())).collect();
        self.list(&items)
    }

    pub fn view(&self, noun: Noun) -> View<'_> {
        match &self.nodes[noun.index()] {
            Node::Atom(bytes) => View::Atom(bytes),
            Node::Cell(h, t) => View::Cell(*h, *t),
        }
    }

    pub fn atom_bytes(&self, noun: Noun) -> Option<&[u8]> {
        match self.view(noun) {
            View::Atom(bytes) => Some(bytes),
            View::Cell(..) => None,
        }
    }

    pub fn as_cell(&self, noun: Noun) -> Option<(Noun, Noun)> {
        match self.view(noun) {
            View::Atom(_) => None,
            View::Cell(h, t) => Some((h, t)),
        }
    }

    pub fn is_atom(&self, noun: Noun) -> bool {
        matches!(self.view(noun), View::Atom(_))
    }

    /// The atom's value if it is an atom that fits in a `u64`.
    pub fn as_u64(&self, noun: Noun) -> Option<u64> {
        let bytes = self.atom_bytes(noun)?;
        if bytes.len() > 8 {
            return None;
        }
        let mut buf = [0u8; 8];
        buf[..bytes.len()].copy_from_slice(bytes);
        Some(u64::from_le_bytes(buf))
    }

    /// Whether `noun` is the atom spelled by `text` (e.g. a mark or tag).
    pub fn is_cord(&self, noun: Noun, text: &str) -> bool {
        self.atom_bytes(noun) == Some(trim(text.as_bytes()))
    }

    /// Elements of a null-terminated list, or `None` if `noun` is not one.
    pub fn list_items(&self, mut noun: Noun) -> Option<Vec<Noun>> {
        let mut items = Vec::new();
        loop {
            match self.view(noun) {
                View::Atom([]) => return Some(items),
                View::Atom(_) => return None,
                View::Cell(h, t) => {
                    items.push(h);
                    noun = t;
                }
            }
        }
    }

    /// Hoon's `dor`: total "tree order" on nouns (atoms before cells, atoms by
    /// value, cells by head then tail). Returns true when `a` sorts at or
    /// before `b`.
    pub fn dor(&self, mut a: Noun, mut b: Noun) -> bool {
        loop {
            if a == b {
                return true;
            }
            match (self.view(a), self.view(b)) {
                (View::Atom(x), View::Atom(y)) => return cmp_atoms(x, y) == Ordering::Less,
                (View::Atom(_), View::Cell(..)) => return true,
                (View::Cell(..), View::Atom(_)) => return false,
                (View::Cell(ah, at), View::Cell(bh, bt)) => {
                    if ah == bh {
                        a = at;
                        b = bt;
                    } else {
                        a = ah;
                        b = bh;
                    }
                }
            }
        }
    }
}

/// Strip insignificant (high) zero bytes from a little-endian atom.
pub fn trim(bytes: &[u8]) -> &[u8] {
    let len = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    &bytes[..len]
}

/// Bit length of a little-endian atom (`met 0`).
pub fn met0(bytes: &[u8]) -> u64 {
    let bytes = trim(bytes);
    match bytes.last() {
        None => 0,
        Some(&top) => (bytes.len() as u64 - 1) * 8 + (8 - top.leading_zeros() as u64),
    }
}

/// Numeric comparison of two little-endian atoms.
pub fn cmp_atoms(a: &[u8], b: &[u8]) -> Ordering {
    let (a, b) = (trim(a), trim(b));
    a.len()
        .cmp(&b.len())
        .then_with(|| a.iter().rev().cmp(b.iter().rev()))
}
