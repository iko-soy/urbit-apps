//! The `.deskjam` format: the little-endian bytes of
//! `(jam [%deskpack-0 desk=@tas (map path page:clay)])`, where a page is
//! `[mark noun]`.

use std::collections::HashSet;
use std::fmt;

use crate::jam::{CueError, cue, jam};
use crate::noun::{Arena, Noun};
use crate::treap::{TreeError, is_valid_treap, map_from_entries, tree_items};

/// The `cask` tag deskjam puts in front of the desk.
pub const TAG: &str = "deskpack-0";

/// A file to pack: its clay path (e.g. `["app", "foo", "hoon"]` for
/// `app/foo.hoon`) and its contents.
pub struct SourceFile {
    pub path: Vec<String>,
    pub contents: Vec<u8>,
}

/// Build a deskjam containing `files`.
///
/// Every file is stored as a `%mime` page, `[%mime /text/plain [size data]]`,
/// which is exactly what vere sends clay when you `|commit` a mounted desk.
/// When deskjam commits the pages, clay converts each one to the mark named
/// by its path extension, using the marks in the target desk, so the
/// resulting desk is identical to one built with `|commit` from the same
/// directory.
pub fn encode(desk: &str, files: Vec<SourceFile>) -> Vec<u8> {
    let mut a = Arena::new();
    let mime = a.cord("mime");
    let mite = a.path(&["text", "plain"]);
    let mut entries = Vec::with_capacity(files.len());
    for file in files {
        let path = a.path(&file.path);
        let size = a.atom_u64(file.contents.len() as u64);
        let data = a.atom(&file.contents);
        let octs = a.cell(size, data);
        let page = a.cell(mite, octs);
        let page = a.cell(mime, page);
        entries.push((path, page));
    }
    let map = map_from_entries(&mut a, &entries);
    let desk = a.cord(desk);
    let tag = a.cord(TAG);
    let body = a.cell(desk, map);
    let root = a.cell(tag, body);
    jam(&a, root)
}

/// One file of a decoded deskjam.
pub struct Page {
    /// Path segments as raw atom bytes.
    pub path: Vec<Vec<u8>>,
    /// The page's mark (raw atom bytes).
    pub mark: Vec<u8>,
    /// The page's noun, in [`Deskpack::arena`].
    pub noun: Noun,
}

/// A decoded deskjam.
pub struct Deskpack {
    pub arena: Arena,
    pub desk: Vec<u8>,
    /// Pages sorted by path.
    pub pages: Vec<Page>,
    /// Whether the file map satisfies the treap invariants. Deskjam's `;;`
    /// rejects files where it does not.
    pub canonical_map: bool,
    /// Whether set bits follow the jam encoding (a ship ignores them).
    pub trailing_garbage: bool,
}

#[derive(Debug)]
pub enum DecodeError {
    Cue(CueError),
    /// The noun is not `[%deskpack-0 ...]`; carries the tag if printable.
    UnknownTag(Option<String>),
    Malformed(String),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Cue(e) => write!(f, "not a jam file: {e}"),
            DecodeError::UnknownTag(Some(tag)) => {
                write!(f, "not a deskjam: expected a %{TAG} jam, found %{tag}")
            }
            DecodeError::UnknownTag(None) => {
                write!(f, "not a deskjam: expected a jam of [%{TAG} desk files]")
            }
            DecodeError::Malformed(why) => write!(f, "malformed deskjam: {why}"),
        }
    }
}

impl std::error::Error for DecodeError {}

impl From<CueError> for DecodeError {
    fn from(e: CueError) -> Self {
        DecodeError::Cue(e)
    }
}

fn malformed(why: impl Into<String>) -> DecodeError {
    DecodeError::Malformed(why.into())
}

/// Parse a deskjam file.
pub fn decode(bytes: &[u8]) -> Result<Deskpack, DecodeError> {
    let mut a = Arena::new();
    let cued = cue(&mut a, bytes)?;
    let (tag, body) = a.as_cell(cued.noun).ok_or(DecodeError::UnknownTag(None))?;
    if !a.is_cord(tag, TAG) {
        let tag = a
            .atom_bytes(tag)
            .and_then(|t| std::str::from_utf8(t).ok())
            .filter(|t| t.chars().all(|c| c.is_ascii_graphic()))
            .map(str::to_owned);
        return Err(DecodeError::UnknownTag(tag));
    }
    let (desk, map) = a
        .as_cell(body)
        .ok_or_else(|| malformed("missing desk name"))?;
    let desk = a
        .atom_bytes(desk)
        .ok_or_else(|| malformed("desk name is not an atom"))?
        .to_vec();

    let items = tree_items(&a, map).map_err(|e| match e {
        TreeError::Malformed => malformed("file table is not a map"),
        TreeError::SharedSubtree => malformed("file table repeats a path"),
    })?;
    let mut seen = HashSet::new();
    let mut pages = Vec::with_capacity(items.len());
    for item in items {
        let (path, page) = a
            .as_cell(item)
            .ok_or_else(|| malformed("file entry is not a pair"))?;
        if !seen.insert(path) {
            return Err(malformed("file table repeats a path"));
        }
        let segments = a
            .list_items(path)
            .ok_or_else(|| malformed("file path is not a list"))?
            .into_iter()
            .map(|s| a.atom_bytes(s).map(<[u8]>::to_vec))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| malformed("file path has a non-atom segment"))?;
        let (mark, noun) = a
            .as_cell(page)
            .ok_or_else(|| malformed("file page is not [mark noun]"))?;
        let mark = a
            .atom_bytes(mark)
            .ok_or_else(|| malformed("file mark is not an atom"))?
            .to_vec();
        pages.push(Page {
            path: segments,
            mark,
            noun,
        });
    }
    pages.sort_by(|x, y| x.path.cmp(&y.path));

    let canonical_map = is_valid_treap(&a, map, |item| a.as_cell(item).map(|(k, _)| k));
    Ok(Deskpack {
        arena: a,
        desk,
        pages,
        canonical_map,
        trailing_garbage: cued.trailing_garbage,
    })
}
