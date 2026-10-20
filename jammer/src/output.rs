//! Writing unpacked files without ever leaving the output directory.
//!
//! Paths come from untrusted deskjam files, and the output directory may be
//! shared with other users, so every component is checked to be a plain
//! name and nothing is followed through a symlink. On Unix each step is
//! taken relative to an open handle of the directory verified in the
//! previous step (`openat` with `O_NOFOLLOW`), so the checks cannot be raced.

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Whether `name` is safe to use as a single path component: a plain name
/// that cannot climb out of its directory, with no control characters (and,
/// on Windows, none of the names and characters Windows reserves).
pub fn is_safe_component(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\'])
        && !name.chars().any(char::is_control)
        && (cfg!(unix) || is_portable(name))
}

/// Windows rejects these characters, trailing dots and spaces, and device
/// names such as `aux` or `com1.txt`.
fn is_portable(name: &str) -> bool {
    const DEVICES: &[&str] = &[
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    !name.contains([':', '*', '?', '"', '<', '>', '|'])
        && !name.ends_with(['.', ' '])
        && !DEVICES.contains(&stem.trim_end())
}

/// Why one file could not be written.
#[derive(Debug)]
pub enum WriteError {
    /// Something else is already where the file or one of its directories
    /// should go (a file written earlier in this run, a symlink, ...).
    Conflict(String),
    Io(io::Error),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Conflict(why) => f.write_str(why),
            WriteError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for WriteError {}

impl From<io::Error> for WriteError {
    fn from(e: io::Error) -> Self {
        WriteError::Io(e)
    }
}

/// Writes files under one output directory.
pub struct Writer {
    root: PathBuf,
    overwrite: bool,
    #[cfg(unix)]
    root_fd: rustix::fd::OwnedFd,
    /// `(device, inode)` of every file written, so two paths that name the
    /// same file (as on case-insensitive filesystems) are caught.
    #[cfg_attr(not(unix), allow(dead_code))]
    written: HashSet<(u64, u64)>,
}

impl Writer {
    /// Use `root` as the output directory: create it if missing, otherwise
    /// insist that it is a directory (a symlink to one is fine) and, unless
    /// `overwrite`, empty. With `overwrite`, existing files are replaced.
    pub fn new(root: &Path, overwrite: bool) -> io::Result<Writer> {
        match fs::metadata(root) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir_all(root)?,
            Err(e) => return Err(e),
            Ok(meta) if !meta.is_dir() => {
                return Err(io::Error::other("it exists and is not a directory"));
            }
            Ok(_) => {
                if !overwrite && fs::read_dir(root)?.next().is_some() {
                    return Err(io::Error::other(
                        "it is not empty (use --force to write into it anyway)",
                    ));
                }
            }
        }
        Ok(Writer {
            root: root.to_path_buf(),
            overwrite,
            #[cfg(unix)]
            root_fd: {
                use rustix::fs::{CWD, Mode, OFlags, openat};
                openat(
                    CWD,
                    root,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                    Mode::empty(),
                )?
            },
            written: HashSet::new(),
        })
    }

    /// Write `contents` to `root/components...`, creating directories as
    /// needed. Returns the path written.
    pub fn write(&mut self, components: &[String], contents: &[u8]) -> Result<PathBuf, WriteError> {
        let (name, dirs) = components
            .split_last()
            .ok_or_else(|| WriteError::Conflict("empty path".into()))?;
        if let Some(bad) = components.iter().find(|c| !is_safe_component(c)) {
            return Err(WriteError::Conflict(format!(
                "{} cannot be a file name",
                crate::text::printable(bad.as_bytes())
            )));
        }
        let path: PathBuf = std::iter::once(self.root.as_path())
            .chain(components.iter().map(Path::new))
            .collect();
        let mut file = self.open(dirs, name, &path)?;
        file.write_all(contents)?;
        Ok(path)
    }

    #[cfg(unix)]
    fn open(&mut self, dirs: &[String], name: &str, path: &Path) -> Result<fs::File, WriteError> {
        use rustix::fs::{FileType, Mode, OFlags, fstat, ftruncate, mkdirat, openat};
        use rustix::io::Errno;

        let mut dir: Option<rustix::fd::OwnedFd> = None;
        let mut at = self.root.clone();
        for component in dirs {
            at.push(component);
            let parent = dir.as_ref().unwrap_or(&self.root_fd);
            match mkdirat(parent, component.as_str(), Mode::from_raw_mode(0o777)) {
                Ok(()) | Err(Errno::EXIST) => {}
                Err(e) => return Err(WriteError::Io(e.into())),
            }
            let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
            dir = Some(
                match openat(parent, component.as_str(), flags, Mode::empty()) {
                    Ok(fd) => fd,
                    Err(Errno::NOTDIR | Errno::LOOP) => {
                        return Err(conflict(&at, "exists and is not a directory"));
                    }
                    Err(e) => return Err(WriteError::Io(e.into())),
                },
            );
        }
        let parent = dir.as_ref().unwrap_or(&self.root_fd);
        let mut flags = OFlags::WRONLY | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        if !self.overwrite {
            flags |= OFlags::EXCL;
        }
        let fd = match openat(parent, name, flags, Mode::from_raw_mode(0o666)) {
            Ok(fd) => fd,
            Err(Errno::EXIST) => return Err(conflict(path, "was already written")),
            Err(Errno::LOOP | Errno::ISDIR) => {
                return Err(conflict(path, "exists and is not a regular file"));
            }
            Err(e) => return Err(WriteError::Io(e.into())),
        };
        let stat = fstat(&fd).map_err(io::Error::from)?;
        if !FileType::from_raw_mode(stat.st_mode).is_file() {
            return Err(conflict(path, "exists and is not a regular file"));
        }
        if !self
            .written
            .insert((stat.st_dev as u64, stat.st_ino as u64))
        {
            return Err(conflict(path, "was already written"));
        }
        if self.overwrite {
            ftruncate(&fd, 0).map_err(io::Error::from)?;
        }
        Ok(fs::File::from(fd))
    }

    /// Without `openat`, check each step and accept the race.
    #[cfg(not(unix))]
    fn open(&mut self, dirs: &[String], _name: &str, path: &Path) -> Result<fs::File, WriteError> {
        let mut at = self.root.clone();
        for component in dirs {
            at.push(component);
            match fs::symlink_metadata(&at) {
                Ok(meta) if meta.is_dir() => {}
                Ok(_) => return Err(conflict(&at, "exists and is not a directory")),
                Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&at)?,
                Err(e) => return Err(e.into()),
            }
        }
        if self.overwrite {
            if let Ok(meta) = fs::symlink_metadata(path)
                && !meta.is_file()
            {
                return Err(conflict(path, "exists and is not a regular file"));
            }
            return Ok(fs::File::create(path)?);
        }
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(file) => Ok(file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                Err(conflict(path, "was already written"))
            }
            Err(e) => Err(e.into()),
        }
    }
}

fn conflict(path: &Path, why: &str) -> WriteError {
    WriteError::Conflict(format!(
        "{} {why}",
        crate::text::printable(path.to_string_lossy().as_bytes())
    ))
}
