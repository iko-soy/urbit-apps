//! Selecting a desk directory's files the way vere does when you `|commit`
//! a mounted desk (`_unix_update_dir` in vere's `io/unix.c`):
//!
//! - names starting with `.` are ignored, files and directories alike;
//! - symlinks are followed;
//! - a file is ignored unless its name contains a `.`, does not end in `~`,
//!   and is a valid `@ta` (`a-z 0-9 - . _ ~`) as a whole;
//! - directory names are not checked;
//! - `dir/name.ext` becomes the clay path `/dir/name/ext`, splitting the
//!   file name at its last `.`;
//! - one leading `!` is dropped from every path segment (and from a file
//!   name before checking it), undoing the escape a desk mount writes for
//!   segments like `..` (see [`crate::marks::unix_path`]).
//!
//! Where vere would produce a commit that clay rejects (an empty
//! extension), hang (a FIFO) or loop forever (a symlink cycle), the file is
//! skipped with a warning instead.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::deskpack::SourceFile;

/// Directories nested deeper than this are not entered (a backstop against
/// symlink cycles the device/inode check cannot see, such as on Windows).
const MAX_DEPTH: usize = 128;

/// Why a directory entry was left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The name starts with `.`.
    Hidden,
    /// The file name has no `.`, so there is no mark for it.
    NoExtension,
    /// The file name ends in `~` (an editor backup).
    Backup,
    /// The file name is not a valid `@ta`.
    InvalidName,
    /// Another file has the same clay path (e.g. `!a.txt` and `a.txt`).
    SamePath(PathBuf),
    /// The output file of this very command.
    Output,
    /// The file name ends in `.`; clay would reject the whole desk.
    EmptyExtension,
    /// The name is not valid UTF-8.
    NotUtf8,
    /// A FIFO, socket or device.
    NotAFile,
    /// It could not be read (a dangling symlink, no permission, ...).
    Unreadable(String),
    /// A symlink to a directory that contains it, or absurdly deep nesting.
    SymlinkLoop,
}

impl SkipReason {
    /// Whether to mention the skip even when not verbose: vere skips the
    /// first few kinds silently too, and they are common in repositories.
    pub fn is_surprising(&self) -> bool {
        !matches!(
            self,
            SkipReason::Hidden | SkipReason::NoExtension | SkipReason::Backup | SkipReason::Output
        )
    }

    /// Whether the file was left out because something is wrong with it,
    /// rather than by the rules `|commit` follows. jam then exits non-zero.
    pub fn is_problem(&self) -> bool {
        matches!(
            self,
            SkipReason::EmptyExtension
                | SkipReason::NotUtf8
                | SkipReason::NotAFile
                | SkipReason::Unreadable(_)
                | SkipReason::SymlinkLoop
        )
    }
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkipReason::Hidden => f.write_str("hidden"),
            SkipReason::NoExtension => f.write_str("no file extension"),
            SkipReason::Backup => f.write_str("backup file"),
            SkipReason::InvalidName => {
                f.write_str("not a valid clay file name (allowed: a-z 0-9 - . _ ~)")
            }
            SkipReason::SamePath(kept) => write!(
                f,
                "it has the same clay path as {}",
                crate::text::printable(kept.to_string_lossy().as_bytes())
            ),
            SkipReason::Output => f.write_str("it is the output file"),
            SkipReason::EmptyExtension => f.write_str("empty file extension"),
            SkipReason::NotUtf8 => f.write_str("name is not valid UTF-8"),
            SkipReason::NotAFile => f.write_str("not a regular file"),
            SkipReason::Unreadable(why) => write!(f, "cannot read it: {why}"),
            SkipReason::SymlinkLoop => f.write_str("symlink loop"),
        }
    }
}

pub struct Skipped {
    pub path: PathBuf,
    pub reason: SkipReason,
}

pub struct Collected {
    /// Files sorted by clay path.
    pub files: Vec<SourceFile>,
    pub skipped: Vec<Skipped>,
    /// Directories whose names are not valid `@ta`s, with the number of
    /// files packed below each. `|commit` includes these too, but such
    /// paths are unusual.
    pub odd_directories: BTreeMap<PathBuf, usize>,
}

/// vere's `_unix_string_to_knot`: drop one leading `!`.
fn unescape(name: &str) -> &str {
    name.strip_prefix('!').unwrap_or(name)
}

/// Whether `name` is how a desk mount spells the segment it stands for
/// (vere's `_unix_knot_to_string`), as opposed to another spelling that
/// `|commit` reads the same way.
fn is_canonical(name: &str) -> bool {
    let knot = unescape(name);
    let escaped = matches!(knot, "" | "." | "..") || knot.starts_with('!');
    name.starts_with('!') == escaped
}

/// Whether `text` is a valid `@ta` (knot): `[a-z0-9-._~]*`.
pub fn is_knot(text: &str) -> bool {
    text.bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-._~".contains(&b))
}

/// Whether `text` is a valid `@tas` (term), as desk names must be:
/// a lowercase letter followed by lowercase letters, digits and hyphens.
pub fn is_term(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[cfg(unix)]
fn dir_id(meta: &fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
fn dir_id(_meta: &fs::Metadata) -> Option<(u64, u64)> {
    None
}

/// Collect the files under `dir` with their clay paths. `exclude` (a
/// canonical path) is never collected.
pub fn collect(dir: &Path, exclude: Option<&Path>) -> io::Result<Collected> {
    struct Pending {
        path: PathBuf,
        clay: Vec<String>,
        /// Directories from the root down to this one, to detect cycles.
        ancestors: Vec<(u64, u64)>,
        /// The outermost directory on the way here whose name is not a knot.
        odd: Option<PathBuf>,
        canonical: bool,
    }
    struct Found {
        file: SourceFile,
        path: PathBuf,
        odd: Option<PathBuf>,
        canonical: bool,
    }
    let mut found = Vec::new();
    let mut skipped = Vec::new();
    let root_meta = fs::metadata(dir)?;
    let mut pending = vec![Pending {
        path: dir.to_path_buf(),
        clay: Vec::new(),
        ancestors: dir_id(&root_meta).into_iter().collect(),
        odd: None,
        canonical: true,
    }];
    while let Some(current) = pending.pop() {
        let entries =
            fs::read_dir(&current.path).and_then(|entries| entries.collect::<io::Result<Vec<_>>>());
        let mut entries = match entries {
            Ok(entries) => entries,
            Err(e) if current.clay.is_empty() => {
                return Err(io::Error::new(
                    e.kind(),
                    format!("{}: {e}", current.path.display()),
                ));
            }
            Err(e) => {
                skipped.push(Skipped {
                    path: current.path,
                    reason: SkipReason::Unreadable(e.to_string()),
                });
                continue;
            }
        };
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let mut skip = |reason| {
                skipped.push(Skipped {
                    path: path.clone(),
                    reason,
                })
            };
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                skip(SkipReason::NotUtf8);
                continue;
            };
            if name.starts_with('.') {
                skip(SkipReason::Hidden);
                continue;
            }
            let meta = match fs::metadata(&path) {
                Ok(meta) => meta,
                Err(e) => {
                    skip(SkipReason::Unreadable(e.to_string()));
                    continue;
                }
            };
            if meta.is_dir() {
                let id = dir_id(&meta);
                if current.clay.len() >= MAX_DEPTH
                    || id.is_some_and(|id| current.ancestors.contains(&id))
                {
                    skip(SkipReason::SymlinkLoop);
                    continue;
                }
                let mut clay = current.clay.clone();
                clay.push(unescape(name).to_owned());
                let mut ancestors = current.ancestors.clone();
                ancestors.extend(id);
                let odd = match &current.odd {
                    Some(odd) => Some(odd.clone()),
                    None if !is_knot(unescape(name)) => Some(path.clone()),
                    None => None,
                };
                pending.push(Pending {
                    path,
                    clay,
                    ancestors,
                    odd,
                    canonical: current.canonical && is_canonical(name),
                });
                continue;
            }
            let Some((stem, ext)) = name.rsplit_once('.') else {
                skip(SkipReason::NoExtension);
                continue;
            };
            if name.ends_with('~') {
                skip(SkipReason::Backup);
                continue;
            }
            if !is_knot(unescape(name)) {
                skip(SkipReason::InvalidName);
                continue;
            }
            if ext.is_empty() {
                skip(SkipReason::EmptyExtension);
                continue;
            }
            if !meta.is_file() {
                skip(SkipReason::NotAFile);
                continue;
            }
            if exclude.is_some_and(|ex| fs::canonicalize(&path).is_ok_and(|p| p == ex)) {
                skip(SkipReason::Output);
                continue;
            }
            let contents = match fs::read(&path) {
                Ok(contents) => contents,
                Err(e) => {
                    skip(SkipReason::Unreadable(e.to_string()));
                    continue;
                }
            };
            let mut clay = current.clay.clone();
            clay.push(unescape(stem).to_owned());
            clay.push(unescape(ext).to_owned());
            found.push(Found {
                file: SourceFile {
                    path: clay,
                    contents,
                },
                odd: current.odd.clone(),
                canonical: current.canonical && is_canonical(stem) && is_canonical(ext),
                path,
            });
        }
    }

    // Two spellings of one clay path: keep the one a mount would write.
    found.sort_by(|a, b| a.file.path.cmp(&b.file.path));
    let mut files: Vec<SourceFile> = Vec::with_capacity(found.len());
    let mut odd_directories = BTreeMap::new();
    let mut group: Vec<Found> = Vec::new();
    let mut found = found.into_iter().peekable();
    while let Some(next) = found.next() {
        group.push(next);
        if found
            .peek()
            .is_some_and(|f| f.file.path == group[0].file.path)
        {
            continue;
        }
        let keep = group.iter().position(|f| f.canonical).unwrap_or(0);
        let kept = group.swap_remove(keep);
        for other in group.drain(..) {
            skipped.push(Skipped {
                path: other.path,
                reason: SkipReason::SamePath(kept.path.clone()),
            });
        }
        if let Some(odd) = kept.odd {
            *odd_directories.entry(odd).or_insert(0) += 1;
        }
        files.push(kept.file);
    }
    skipped.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Collected {
        files,
        skipped,
        odd_directories,
    })
}

/// Files clay parses as Hoon (`sys.kelvin`, `desk.bill`, `desk.docket-0`)
/// that contain carriage returns or tabs, which Hoon's parser rejects: a
/// commit including them fails.
pub fn unparsable_config(files: &[SourceFile]) -> Vec<String> {
    files
        .iter()
        .filter(|f| {
            matches!(
                f.path.last().map(String::as_str),
                Some("kelvin" | "bill" | "docket-0")
            ) && f.contents.iter().any(|&b| b == b'\r' || b == b'\t')
        })
        .map(|f| f.path.join("/"))
        .collect()
}

/// Marks used by `files` that have no definition among them. Clay builds
/// marks from the desk itself, so a new desk made from these files could not
/// convert them. Clay converts `%hoon`, `%mime` and `%noun` without a mark
/// file, and `%jam` with `mar/mime.hoon`.
pub fn missing_marks(files: &[SourceFile]) -> BTreeSet<String> {
    // `/mar/foo/bar/hoon` defines %foo-bar (as does `/mar/foo-bar/hoon`).
    let defined: HashSet<String> = files
        .iter()
        .filter_map(|f| match f.path.as_slice() {
            [mar, name @ .., ext] if mar == "mar" && ext == "hoon" && !name.is_empty() => {
                Some(name.join("-"))
            }
            _ => None,
        })
        .collect();
    let used: BTreeSet<&String> = files.iter().filter_map(|f| f.path.last()).collect();
    used.into_iter()
        .filter(|mark| !matches!(mark.as_str(), "hoon" | "mime" | "noun"))
        .filter(|mark| {
            !defined.contains(mark.as_str())
                && !(mark.as_str() == "jam" && defined.contains("mime"))
        })
        .cloned()
        .collect()
}
