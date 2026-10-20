//! Turning clay pages back into file contents, like a desk mount does with
//! each mark's `grow:mime` arm.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::aura;
use crate::jam::jam_counting;
use crate::noun::{Arena, Noun, View, cmp_atoms, trim};
use crate::output::is_safe_component;
use crate::text::printable;

/// Why a page could not be turned into file contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkError {
    /// The noun does not have the shape the mark requires.
    BadNoun(String),
    /// A mark whose pages jammer cannot render.
    Unsupported(String),
    /// The file would be larger than [`MAX_FILE_SIZE`].
    TooLarge,
}

/// Largest file `to_bytes` will produce. Backreferences let a small deskjam
/// describe enormous files, so untrusted input must be bounded.
pub const MAX_FILE_SIZE: usize = 1 << 30;

impl fmt::Display for MarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarkError::BadNoun(mark) => write!(f, "contents are not a valid %{mark} noun"),
            MarkError::Unsupported(mark) => {
                write!(f, "jammer cannot turn a %{mark} page into a file")
            }
            MarkError::TooLarge => write!(f, "file would be larger than {} bytes", MAX_FILE_SIZE),
        }
    }
}

impl std::error::Error for MarkError {}

/// The relative unix path a desk mount uses for a clay path:
/// `/app/foo/hoon` becomes `app/foo.hoon`. Like vere, segments that are
/// empty, `.` or `..`, or that start with `!`, are written with a `!` in
/// front (which `|commit` strips again), so they cannot escape the
/// directory. Segments need not be valid `@ta`s (`|commit` accepts directory
/// names like `Upper`), but they must be UTF-8 without separators or
/// control characters.
pub fn unix_path(segments: &[Vec<u8>]) -> Result<Vec<String>, String> {
    let mut parts = Vec::with_capacity(segments.len());
    for segment in segments {
        let text = std::str::from_utf8(segment)
            .ok()
            .map(|s| match s {
                "" | "." | ".." => format!("!{s}"),
                s if s.starts_with('!') => format!("!{s}"),
                s => s.to_owned(),
            })
            .filter(|s| is_safe_component(s))
            .ok_or_else(|| {
                format!(
                    "path segment {:?} cannot be a file name",
                    String::from_utf8_lossy(segment)
                )
            })?;
        parts.push(text);
    }
    match parts.len() {
        0 => Err("empty path".into()),
        1 => Err("path has no file extension".into()),
        _ => {
            let mark = parts.pop().unwrap();
            let stem = parts.pop().unwrap();
            parts.push(format!("{stem}.{mark}"));
            Ok(parts)
        }
    }
}

/// Marks in `%base` and landscape whose files are not just the noun's bytes,
/// and which jammer does not render: XML (`manx`) and CSV printers, and
/// marks with no file form at all (a desk mount crashes on them).
const UNRENDERABLE: &[&[u8]] = &[
    b"hymn",
    b"urb",
    b"snip",
    b"eth-txs",
    b"acme-order",
    b"aqua-effect",
    b"azimuth-update",
    b"belt",
    b"blit",
    b"dill-belt",
    b"dill-blit",
    b"dns-address",
    b"dns-binding",
    b"dns-complete",
    b"dns-request",
    b"drum-put",
    b"egg-any",
    b"eth-watcher-poke",
    b"helm-moon-breach",
    b"herm-task",
    b"httr",
    b"json-rpc-response",
    b"kiln-approve-merge",
    b"kiln-bump",
    b"kiln-install",
    b"kiln-jump",
    b"kiln-jump-ask",
    b"kiln-jump-opt",
    b"kiln-pause",
    b"kiln-pikes",
    b"kiln-resume",
    b"kiln-revive",
    b"kiln-suspend",
    b"kiln-sync",
    b"kiln-sync-update",
    b"kiln-uninstall",
    b"kiln-unsync",
    b"language-server-rpc-notification",
    b"language-server-rpc-request",
    b"language-server-rpc-response",
    b"lens-command",
    b"loob",
    b"mass",
    b"path",
    b"purl",
    b"sole-action",
    b"sole-effect",
    b"spider-stop",
    b"story-diff",
    b"tang",
    b"ted-eval",
    b"thread-done",
    b"thread-fail",
    b"txt-diff",
    b"urbit",
    b"verb-event",
    b"verb-event-plus",
    b"vere",
    b"vere-update",
    b"version",
];

/// Marks whose files are simply the bytes of their atom (`grow:mime` is
/// `(as-octs atom)`), from `%base`, landscape and `%groups`.
const RAW_BYTES: &[&[u8]] = &[
    b"hoon", b"js", b"css", b"html", b"htm", b"xhtml", b"svg", b"png", b"ico", b"map", b"jam",
    b"xml", b"udon", b"umd", b"atom", b"helm-hi", b"release",
];

/// Whether jammer knows how pages of `mark` render, rather than guessing
/// that the file is the bytes of the page's atom (as it does for marks it
/// has never heard of).
pub fn is_known(mark: &[u8]) -> bool {
    RAW_BYTES.contains(&mark)
        || UNRENDERABLE.contains(&mark)
        || matches!(
            mark,
            b"mime"
                | b"wasm"
                | b"woff2"
                | b"txt"
                | b"md"
                | b"pem"
                | b"tape"
                | b"story"
                | b"json"
                | b"kelvin"
                | b"bill"
                | b"docket-0"
                | b"ship"
                | b"run-check"
                | b"noun"
                | b"noun-autodiff"
                | b"azimuth-snapshot"
                | b"eth-logs"
                | b"pill"
        )
}

/// Why a renderer failed; [`to_bytes`] adds the mark name.
enum Fail {
    Bad,
    TooLarge,
}

/// The cost of walking one noun, in bytes of output. Walking is what a
/// hostile page can make expensive without producing anything (a long list
/// of empty lines, a big map that turns out to be malformed).
pub const VISIT_COST: u64 = 64;

/// A file under construction that refuses to grow past [`MAX_FILE_SIZE`],
/// so hostile nouns (say, a list repeating one big atom) cannot make a
/// renderer allocate more than that. It also counts the nouns the renderer
/// walks, so callers can bound the total work.
#[derive(Default)]
struct Out {
    bytes: Vec<u8>,
    visits: u64,
}

impl Out {
    fn put(&mut self, bytes: &[u8]) -> Result<(), Fail> {
        if self.bytes.len() + bytes.len() > MAX_FILE_SIZE {
            return Err(Fail::TooLarge);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn atom(&mut self, arena: &Arena, noun: Noun) -> Result<(), Fail> {
        self.put(arena.atom_bytes(noun).ok_or(Fail::Bad)?)
    }

    fn cost(&self) -> u64 {
        self.bytes.len() as u64 + VISIT_COST * self.visits
    }

    /// The elements of a null-terminated list.
    fn list(&mut self, arena: &Arena, mut noun: Noun) -> Result<Vec<Noun>, Fail> {
        let mut items = Vec::new();
        loop {
            self.visits += 1;
            match arena.view(noun) {
                View::Atom([]) => return Ok(items),
                View::Atom(_) => return Err(Fail::Bad),
                View::Cell(head, tail) => {
                    items.push(head);
                    noun = tail;
                }
            }
        }
    }

    /// The items of a map or set tree in `tap` order (right subtree, node,
    /// left subtree). A repeated subtree is malformed, which also keeps
    /// the walk linear.
    fn tap(&mut self, arena: &Arena, tree: Noun) -> Result<Vec<Noun>, Fail> {
        let mut items = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = Vec::new();
        let mut node = tree;
        loop {
            loop {
                self.visits += 1;
                match arena.view(node) {
                    View::Atom([]) => break,
                    View::Atom(_) => return Err(Fail::Bad),
                    View::Cell(_, children) => {
                        if !seen.insert(node) {
                            return Err(Fail::Bad);
                        }
                        stack.push(node);
                        node = arena.as_cell(children).ok_or(Fail::Bad)?.1;
                    }
                }
            }
            let Some(top) = stack.pop() else { break };
            let (item, children) = arena.as_cell(top).ok_or(Fail::Bad)?;
            items.push(item);
            node = arena.as_cell(children).ok_or(Fail::Bad)?.0;
        }
        Ok(items)
    }
}

/// Largest atom printed as `@ud` (617 digits): decimal conversion is
/// quadratic, and the numbers marks print (kelvins, versions) are tiny.
const MAX_UD_BYTES: usize = 256;

fn ud(arena: &Arena, noun: Noun) -> Result<String, Fail> {
    let atom = arena.atom_bytes(noun).ok_or(Fail::Bad)?;
    if atom.len() > MAX_UD_BYTES {
        return Err(Fail::TooLarge);
    }
    Ok(aura::ud(atom))
}

/// `@ux`, `@uv` and `@p` text is at most a few times the atom's size.
fn other_aura(arena: &Arena, noun: Noun, print: fn(&[u8]) -> String) -> Result<String, Fail> {
    let atom = arena.atom_bytes(noun).ok_or(Fail::Bad)?;
    if atom.len() > MAX_FILE_SIZE / 4 {
        return Err(Fail::TooLarge);
    }
    Ok(print(atom))
}

/// A rendered page, and what rendering it cost: the bytes produced (as far
/// as it got) plus [`VISIT_COST`] for each noun walked, whether or not it
/// succeeded.
pub struct Rendered {
    pub result: Result<Vec<u8>, MarkError>,
    pub cost: u64,
}

/// File contents for a page with the given mark.
pub fn to_bytes(arena: &Arena, mark: &[u8], noun: Noun) -> Result<Vec<u8>, MarkError> {
    render(arena, mark, noun).result
}

/// [`to_bytes`], with the cost of rendering.
pub fn render(arena: &Arena, mark: &[u8], noun: Noun) -> Rendered {
    let name = || printable(mark);
    let octs = |octs| {
        let result = octs_bytes(arena, octs).map_err(|e| fail(e, name));
        let cost = result.as_ref().map_or(0, |bytes| bytes.len() as u64);
        Rendered { result, cost }
    };
    let unsupported = || Rendered {
        result: Err(MarkError::Unsupported(name())),
        cost: 0,
    };
    let mut out = Out::default();
    let rendered = match mark {
        b"mime" => match arena.as_cell(noun) {
            Some((_, data)) => return octs(data),
            None => Err(Fail::Bad),
        },
        // Binary marks whose noun is `octs`.
        b"wasm" | b"woff2" => return octs(noun),
        b"txt" | b"md" | b"pem" => txt(arena, noun, &mut out),
        b"tape" => tape(arena, noun, &mut out),
        b"story" => story(arena, noun, &mut out),
        b"json" => json(arena, noun, &mut out),
        b"kelvin" => kelvin(arena, noun, &mut out),
        // These two print through tapes, and `crip` drops NUL bytes.
        b"bill" => bill(arena, noun, &mut out),
        b"docket-0" => docket(arena, noun, &mut out),
        b"ship" | b"run-check" => {
            other_aura(arena, noun, aura::p).and_then(|text| out.put(text.as_bytes()))
        }
        // Generic noun marks write the noun's jam.
        b"noun" | b"noun-autodiff" | b"azimuth-snapshot" | b"eth-logs" | b"pill" => {
            let (bytes, visits) = jam_counting(arena, noun);
            out.bytes = bytes;
            // jam hashes every noun it visits: count those visits as dearer.
            out.visits = 4 * visits;
            Ok(())
        }
        _ if UNRENDERABLE.contains(&mark) => return unsupported(),
        // Most marks (hoon, js, css, html, svg, png, ...) store the file
        // itself as an atom.
        _ => match arena.atom_bytes(noun) {
            Some(bytes) => out.put(bytes),
            None => return unsupported(),
        },
    };
    let cost = out.cost();
    let result = match rendered {
        Err(e) => Err(fail(e, name)),
        Ok(()) if out.bytes.len() > MAX_FILE_SIZE => Err(MarkError::TooLarge),
        Ok(()) => {
            let mut bytes = out.bytes;
            if matches!(mark, b"bill" | b"docket-0") {
                bytes.retain(|&b| b != 0);
            }
            Ok(bytes)
        }
    };
    Rendered { result, cost }
}

fn fail(fail: Fail, name: impl Fn() -> String) -> MarkError {
    match fail {
        Fail::Bad => MarkError::BadNoun(name()),
        Fail::TooLarge => MarkError::TooLarge,
    }
}

/// The size an `octs`-based page (all pages jammer writes) declares, found
/// without rendering it; `None` for other pages. Too-large sizes are
/// reported as [`MarkError::TooLarge`].
pub fn declared_size(arena: &Arena, mark: &[u8], noun: Noun) -> Option<Result<usize, MarkError>> {
    let octs = match mark {
        b"mime" => arena.as_cell(noun)?.1,
        b"wasm" | b"woff2" => noun,
        _ => return None,
    };
    match octs_parts(arena, octs) {
        Ok((size, _)) => Some(Ok(size)),
        Err(Fail::TooLarge) => Some(Err(MarkError::TooLarge)),
        Err(Fail::Bad) => None,
    }
}

/// The size of the file [`to_bytes`] would produce, cheaply where possible.
pub fn size(arena: &Arena, mark: &[u8], noun: Noun) -> Result<usize, MarkError> {
    declared_size(arena, mark, noun)
        .unwrap_or_else(|| to_bytes(arena, mark, noun).map(|bytes| bytes.len()))
}

/// `[size data]`: the first `size` bytes of `data`, zero-padded, as vere
/// writes a `mime`'s `octs`.
fn octs_bytes(arena: &Arena, octs: Noun) -> Result<Vec<u8>, Fail> {
    let (size, data) = octs_parts(arena, octs)?;
    let mut out = data[..data.len().min(size)].to_vec();
    out.resize(size, 0);
    Ok(out)
}

fn octs_parts(arena: &Arena, octs: Noun) -> Result<(usize, &[u8]), Fail> {
    let (size, data) = arena.as_cell(octs).ok_or(Fail::Bad)?;
    let data = arena.atom_bytes(data).ok_or(Fail::Bad)?;
    match arena.as_u64(size) {
        Some(size) if size <= MAX_FILE_SIZE as u64 => Ok((size as usize, data)),
        Some(_) => Err(Fail::TooLarge),
        None if arena.is_atom(size) => Err(Fail::TooLarge),
        None => Err(Fail::Bad),
    }
}

/// `%txt`: a `wain`, printed by `of-wain` (lines joined by newlines).
fn txt(arena: &Arena, noun: Noun, out: &mut Out) -> Result<(), Fail> {
    for (i, line) in out.list(arena, noun)?.into_iter().enumerate() {
        if i > 0 {
            out.put(b"\n")?;
        }
        out.atom(arena, line)?;
    }
    Ok(())
}

/// `%tape`: a list of characters, concatenated (`rap 3`).
fn tape(arena: &Arena, noun: Noun, out: &mut Out) -> Result<(), Fail> {
    for char in out.list(arena, noun)? {
        out.atom(arena, char)?;
    }
    Ok(())
}

/// `to-wain` as ships run it (the `leer` jet): split at newlines, where a
/// final newline ends the last line rather than starting an empty one, and
/// each line is an atom (losing trailing NUL bytes).
fn to_wain(text: &[u8]) -> Vec<&[u8]> {
    let text = trim(text);
    let mut lines = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = text[start..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(text.len(), |i| start + i);
        lines.push(trim(&text[start..end]));
        start = end + 1;
    }
    lines
}

/// `%story`: commit messages, `(jug tako [title=@t body=@t])`, printed by
/// `of-wain`. Each chapter is `commit: <tako>` and its proses (`title`, blank
/// line, `body`, through `to-wain`), proses separated by `|||` and chapters
/// by `---`.
fn story(arena: &Arena, noun: Noun, out: &mut Out) -> Result<(), Fail> {
    let mut first_line = true;
    let mut line = |out: &mut Out, text: &[u8]| {
        if !first_line {
            out.put(b"\n")?;
        }
        first_line = false;
        out.put(text)
    };
    let mut first_chapter = true;
    for chapter in out.tap(arena, noun)? {
        let (tako, proses) = arena.as_cell(chapter).ok_or(Fail::Bad)?;
        let proses = out.tap(arena, proses)?;
        if proses.is_empty() {
            continue;
        }
        if !first_chapter {
            line(out, b"---")?;
        }
        first_chapter = false;
        let tako = other_aura(arena, tako, aura::uv)?;
        line(out, format!("commit: {tako}").as_bytes())?;
        for (i, prose) in proses.into_iter().enumerate() {
            if i > 0 {
                line(out, b"|||")?;
            }
            let (title, body) = arena.as_cell(prose).ok_or(Fail::Bad)?;
            let title = arena.atom_bytes(title).ok_or(Fail::Bad)?;
            let body = arena.atom_bytes(body).ok_or(Fail::Bad)?;
            // `(rap 3 title '\0a\0a' body ~)`
            if title.len() + body.len() > MAX_FILE_SIZE {
                return Err(Fail::TooLarge);
            }
            let text = [title, b"\n\n", body].concat();
            for part in to_wain(&text) {
                line(out, part)?;
            }
        }
    }
    Ok(())
}

/// `%kelvin`: a `waft` (one `[lal num]` weft, or `[[%1 ~] (set weft)]`),
/// printed one `[%lal num]` per line, highest first.
fn kelvin(arena: &Arena, noun: Noun, out: &mut Out) -> Result<(), Fail> {
    let (head, tail) = arena.as_cell(noun).ok_or(Fail::Bad)?;
    let wefts = if arena.is_atom(head) {
        vec![noun]
    } else {
        out.tap(arena, tail)?
    };
    let mut wefts = wefts
        .into_iter()
        .map(|weft| {
            let (lal, num) = arena.as_cell(weft).ok_or(Fail::Bad)?;
            arena.atom_bytes(lal).ok_or(Fail::Bad)?;
            arena.atom_bytes(num).ok_or(Fail::Bad)?;
            Ok((lal, num))
        })
        .collect::<Result<Vec<_>, Fail>>()?;
    let order = |a: Noun, b: Noun| {
        if a == b {
            std::cmp::Ordering::Equal
        } else {
            cmp_atoms(arena.atom_bytes(a).unwrap(), arena.atom_bytes(b).unwrap())
        }
    };
    wefts.sort_by(|a, b| order(b.0, a.0).then_with(|| order(b.1, a.1)));
    // Many wefts can share one number; print each number once.
    let mut printed: HashMap<Noun, String> = HashMap::new();
    for (lal, num) in wefts {
        let num = match printed.get(&num) {
            Some(text) => text,
            None => printed.entry(num).or_insert(ud(arena, num)?),
        };
        out.put(b"[%")?;
        out.atom(arena, lal)?;
        out.put(b" ")?;
        out.put(num.as_bytes())?;
        out.put(b"]\n")?;
    }
    Ok(())
}

/// `%bill`: a list of agent names, printed as a `:~` list.
fn bill(arena: &Arena, noun: Noun, out: &mut Out) -> Result<(), Fail> {
    let dudes = out.list(arena, noun)?;
    if dudes.is_empty() {
        return out.put(b"~\n");
    }
    for (i, dude) in dudes.into_iter().enumerate() {
        out.put(if i == 0 { b":~  %" } else { b"    %" })?;
        out.atom(arena, dude)?;
        out.put(b"\n")?;
    }
    out.put(b"==\n")
}

/// `%docket-0`: landscape's app metadata, printed by `spit-docket`.
fn docket(arena: &Arena, noun: Noun, out: &mut Out) -> Result<(), Fail> {
    let fields = tuple(arena, noun, 9).ok_or(Fail::Bad)?;
    let [
        tag,
        title,
        info,
        color,
        href,
        image,
        version,
        website,
        license,
    ] = fields[..]
    else {
        return Err(Fail::Bad);
    };
    if arena.as_u64(tag) != Some(1) {
        return Err(Fail::Bad);
    }
    let [major, minor, patch] = tuple(arena, version, 3).ok_or(Fail::Bad)?[..] else {
        return Err(Fail::Bad);
    };
    let (kind, rest) = arena.as_cell(href).ok_or(Fail::Bad)?;
    let quoted = |out: &mut Out, key: &str, value: Noun| {
        out.put(format!("\n  {key}+'").as_bytes())?;
        out.atom(arena, value)?;
        out.put(b"'")
    };

    out.put(b":~")?;
    quoted(out, "title", title)?;
    quoted(out, "info", info)?;
    let color = other_aura(arena, color, aura::ux)?;
    out.put(format!("\n  color+{color}").as_bytes())?;
    let version = format!(
        "\n  version+[{} {} {}]",
        ud(arena, major)?,
        ud(arena, minor)?,
        ud(arena, patch)?
    );
    out.put(version.as_bytes())?;
    quoted(out, "website", website)?;
    quoted(out, "license", license)?;
    // image: (unit url)
    if !arena.is_cord(image, "") {
        let (null, url) = arena.as_cell(image).ok_or(Fail::Bad)?;
        if !arena.is_cord(null, "") {
            return Err(Fail::Bad);
        }
        quoted(out, "image", url)?;
    }
    if arena.is_cord(kind, "site") {
        out.put(b"\n  site+")?;
        spud(arena, rest, out)?;
    } else if arena.is_cord(kind, "glob") {
        let (base, reference) = arena.as_cell(rest).ok_or(Fail::Bad)?;
        let (hash, location) = arena.as_cell(reference).ok_or(Fail::Bad)?;
        let (how, place) = arena.as_cell(location).ok_or(Fail::Bad)?;
        let hash = other_aura(arena, hash, aura::uv)?;
        quoted(out, "base", base)?;
        if arena.is_cord(how, "http") {
            out.put(b"\n  glob-http+['")?;
            out.atom(arena, place)?;
            out.put(format!("' {hash}]").as_bytes())?;
        } else if arena.is_cord(how, "ames") {
            let ship = other_aura(arena, place, aura::p)?;
            out.put(format!("\n  glob-ames+[{ship} {hash}]").as_bytes())?;
        } else {
            return Err(Fail::Bad);
        }
    } else {
        return Err(Fail::Bad);
    }
    out.put(b"\n==")
}

/// Split a noun into an `n`-tuple `[a b ... z]`.
fn tuple(arena: &Arena, mut noun: Noun, n: usize) -> Option<Vec<Noun>> {
    let mut items = Vec::with_capacity(n);
    for _ in 1..n {
        let (head, tail) = arena.as_cell(noun)?;
        items.push(head);
        noun = tail;
    }
    items.push(noun);
    Some(items)
}

/// `spud`: render a path as `/a/b/c` (`/` for the empty path).
fn spud(arena: &Arena, path: Noun, out: &mut Out) -> Result<(), Fail> {
    let segments = out.list(arena, path)?;
    if segments.is_empty() {
        return out.put(b"/");
    }
    for segment in segments {
        out.put(b"/")?;
        out.atom(arena, segment)?;
    }
    Ok(())
}

/// `%json`: printed by `en:json:html`.
fn json(arena: &Arena, noun: Noun, out: &mut Out) -> Result<(), Fail> {
    enum Task {
        Value(Noun),
        Text(&'static [u8]),
        Key(Noun),
    }
    let mut tasks = vec![Task::Value(noun)];
    while let Some(task) = tasks.pop() {
        let value = match task {
            Task::Text(text) => {
                out.put(text)?;
                continue;
            }
            Task::Key(key) => {
                json_string(out, arena.atom_bytes(key).ok_or(Fail::Bad)?)?;
                continue;
            }
            Task::Value(value) => value,
        };
        out.visits += 1;
        let Some((tag, body)) = arena.as_cell(value) else {
            // `~` is null; no other atom is valid json.
            if arena.is_cord(value, "") {
                out.put(b"null")?;
                continue;
            }
            return Err(Fail::Bad);
        };
        match arena.atom_bytes(tag).ok_or(Fail::Bad)? {
            b"a" => {
                let items = out.list(arena, body)?;
                if items.is_empty() {
                    out.put(b"[]")?;
                    continue;
                }
                out.put(b"[")?;
                tasks.push(Task::Text(b"]"));
                for (i, &item) in items.iter().enumerate().rev() {
                    tasks.push(Task::Value(item));
                    if i > 0 {
                        tasks.push(Task::Text(b","));
                    }
                }
            }
            b"b" => match arena.as_u64(body) {
                Some(0) => out.put(b"true")?,
                Some(1) => out.put(b"false")?,
                _ => return Err(Fail::Bad),
            },
            b"n" => out.atom(arena, body)?,
            b"s" => json_string(out, arena.atom_bytes(body).ok_or(Fail::Bad)?)?,
            b"o" => {
                // `~(tap by map)` order: right subtree, node, left subtree.
                let entries = out.tap(arena, body)?;
                if entries.is_empty() {
                    out.put(b"{}")?;
                    continue;
                }
                out.put(b"{")?;
                tasks.push(Task::Text(b"}"));
                for (i, &entry) in entries.iter().enumerate().rev() {
                    let (key, item) = arena.as_cell(entry).ok_or(Fail::Bad)?;
                    tasks.push(Task::Value(item));
                    tasks.push(Task::Text(b":"));
                    tasks.push(Task::Key(key));
                    if i > 0 {
                        tasks.push(Task::Text(b","));
                    }
                }
            }
            _ => return Err(Fail::Bad),
        }
    }
    Ok(())
}

/// `scap:en:json:html`: a JSON string literal. Bytes above 0x1f pass through
/// untouched (so UTF-8 stays UTF-8); newline, quote and backslash get short
/// escapes and other control bytes `\u00xx`.
fn json_string(out: &mut Out, text: &[u8]) -> Result<(), Fail> {
    out.put(b"\"")?;
    // Escaping at most sextuples a byte; check the budget per chunk.
    for chunk in text.chunks(1 << 16) {
        let mut escaped = Vec::with_capacity(chunk.len() + 16);
        for &byte in chunk {
            match byte {
                b'\n' => escaped.extend_from_slice(b"\\n"),
                b'"' => escaped.extend_from_slice(b"\\\""),
                b'\\' => escaped.extend_from_slice(b"\\\\"),
                0..=0x1f | 0x7f => escaped.extend_from_slice(format!("\\u{byte:04x}").as_bytes()),
                _ => escaped.push(byte),
            }
        }
        out.put(&escaped)?;
    }
    out.put(b"\"")
}
