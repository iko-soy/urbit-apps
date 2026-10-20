//! Hoon maps and sets.
//!
//! Both are treaps: a binary search tree ordered by [`Arena::gor`] whose
//! nodes are heap-ordered by [`Arena::mor`]. A node is `[n=item l=tree r=tree]`
//! and the empty tree is `~`. Since both orders are total, the treap for a
//! given key set is unique, so building it directly produces exactly the noun
//! a ship builds with `put:by`/`gas:by` (and passes the `apt:by` check that
//! `;;` runs for map molds).

use std::collections::HashSet;
use std::fmt;

use crate::noun::{Arena, Noun, View};

/// Build `(map k v)` from `[key value]` entries, like `(malt entries)`.
/// If a key repeats, the last value wins (as with `gas:by`).
pub fn map_from_entries(arena: &mut Arena, entries: &[(Noun, Noun)]) -> Noun {
    let items = entries
        .iter()
        .map(|&(key, value)| (key, arena.cell(key, value)))
        .collect();
    build(arena, items)
}

/// Build `(set item)` from members, like `(silt items)`.
pub fn set_from_items(arena: &mut Arena, items: &[Noun]) -> Noun {
    build(arena, items.iter().map(|&item| (item, item)).collect())
}

/// `items` are `(key, node item)` pairs.
fn build(arena: &mut Arena, mut items: Vec<(Noun, Noun)>) -> Noun {
    // Stable sort, then keep the last of each run of equal keys.
    items.sort_by(|a, b| arena.gor_cmp(a.0, b.0));
    let mut unique: Vec<(Noun, Noun)> = Vec::with_capacity(items.len());
    for item in items {
        match unique.last_mut() {
            Some(last) if last.0 == item.0 => *last = item,
            _ => unique.push(item),
        }
    }

    // Cartesian tree over the gor-sorted keys, heap-ordered by mor.
    const NONE: usize = usize::MAX;
    let mut left = vec![NONE; unique.len()];
    let mut right = vec![NONE; unique.len()];
    let mut spine: Vec<usize> = Vec::new();
    for i in 0..unique.len() {
        let mut last = NONE;
        while let Some(&top) = spine.last() {
            if !arena.mor(unique[i].0, unique[top].0) {
                break;
            }
            last = top;
            spine.pop();
        }
        left[i] = last;
        if let Some(&top) = spine.last() {
            right[top] = i;
        }
        spine.push(i);
    }

    // Emit nouns bottom-up (post-order, explicit stack).
    let root = spine.first().copied().unwrap_or(NONE);
    let null = arena.atom(&[]);
    let mut built = vec![null; unique.len()];
    let mut stack = vec![(root, false)];
    while let Some((i, children_done)) = stack.pop() {
        if i == NONE {
            continue;
        }
        if !children_done {
            stack.push((i, true));
            stack.push((left[i], false));
            stack.push((right[i], false));
            continue;
        }
        let l = if left[i] == NONE {
            null
        } else {
            built[left[i]]
        };
        let r = if right[i] == NONE {
            null
        } else {
            built[right[i]]
        };
        let children = arena.cell(l, r);
        built[i] = arena.cell(unique[i].1, children);
    }
    if root == NONE { null } else { built[root] }
}

/// A noun that is not shaped like a tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeError {
    /// A node is neither `~` nor `[n l r]`.
    Malformed,
    /// The same subtree occurs twice, so keys repeat (and naive traversal
    /// could take exponential time).
    SharedSubtree,
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeError::Malformed => write!(f, "not a well-formed map/set tree"),
            TreeError::SharedSubtree => write!(f, "map/set tree contains a repeated subtree"),
        }
    }
}

impl std::error::Error for TreeError {}

/// The node items (`n`) of a tree, in pre-order.
pub fn tree_items(arena: &Arena, tree: Noun) -> Result<Vec<Noun>, TreeError> {
    let mut items = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = vec![tree];
    while let Some(node) = stack.pop() {
        match arena.view(node) {
            View::Atom([]) => {}
            View::Atom(_) => return Err(TreeError::Malformed),
            View::Cell(item, children) => {
                if !visited.insert(node) {
                    return Err(TreeError::SharedSubtree);
                }
                let (l, r) = arena.as_cell(children).ok_or(TreeError::Malformed)?;
                items.push(item);
                stack.push(r);
                stack.push(l);
            }
        }
    }
    Ok(items)
}

/// Hoon's `apt:by` / `apt:in`: whether a well-formed tree satisfies the treap
/// invariants. `key` extracts the ordering key from a node item (the head of
/// a map entry, or the item itself for sets). Returns false for malformed
/// trees.
pub fn is_valid_treap(arena: &Arena, tree: Noun, key: impl Fn(Noun) -> Option<Noun>) -> bool {
    // (node, upper bound from a left descent, lower bound from a right descent)
    let mut stack: Vec<(Noun, Option<Noun>, Option<Noun>)> = vec![(tree, None, None)];
    let node_key = |node: Noun| -> Option<(Noun, Noun, Noun)> {
        let (item, children) = arena.as_cell(node)?;
        let (l, r) = arena.as_cell(children)?;
        Some((key(item)?, l, r))
    };
    let is_null = |n: Noun| arena.atom_bytes(n) == Some(&[]);
    let mut visited = HashSet::new();
    while let Some((node, upper, lower)) = stack.pop() {
        if is_null(node) {
            continue;
        }
        if !visited.insert(node) {
            return false;
        }
        let Some((k, l, r)) = node_key(node) else {
            return false;
        };
        if let Some(u) = upper
            && (k == u || !arena.gor(k, u))
        {
            return false;
        }
        if let Some(w) = lower
            && (w == k || !arena.gor(w, k))
        {
            return false;
        }
        for (child, is_left) in [(l, true), (r, false)] {
            if is_null(child) {
                continue;
            }
            let Some((ck, _, _)) = node_key(child) else {
                return false;
            };
            if ck == k || !arena.mor(k, ck) {
                return false;
            }
            if is_left {
                stack.push((child, Some(k), lower));
            } else {
                stack.push((child, upper, Some(k)));
            }
        }
    }
    true
}
