use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use jammer::deskpack::{self, Deskpack, Page};
use jammer::marks::{self, MarkError};
use jammer::noun::Noun;
use jammer::output::{WriteError, Writer};
use jammer::pack;
use jammer::text::printable;

#[derive(Parser)]
#[command(
    version,
    about = "Pack desk directories into .deskjam files for the deskjam Urbit app, and unpack them",
    after_help = "Exit status: 0 on success, 1 on error, 3 if some files were left out."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pack a desk directory into a .deskjam file
    ///
    /// Files are selected exactly like `|commit` selects them from a mounted
    /// desk, and stored so that deskjam builds the same desk `|commit` would.
    Jam {
        /// The desk directory (a mounted desk, or a repo's desk folder)
        dir: PathBuf,
        /// Output file [default: ./<desk>.deskjam]; `-` writes to stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Desk name recorded in the file [default: the directory's name]
        #[arg(short, long)]
        desk: Option<String>,
        /// Replace an existing output file that is not a .deskjam
        #[arg(short, long)]
        force: bool,
        /// Print every file that is packed or skipped
        #[arg(short, long)]
        verbose: bool,
    },
    /// Unpack a .deskjam file into a directory
    ///
    /// Each file is converted to the bytes a desk mount would write for it.
    Unjam {
        /// The .deskjam file; `-` reads stdin
        file: PathBuf,
        /// Output directory [default: ./<desk>]
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Write into an existing, non-empty directory, overwriting files
        #[arg(short, long)]
        force: bool,
        /// Print every file as it is written
        #[arg(short, long)]
        verbose: bool,
    },
    /// List the files in a .deskjam file
    #[command(visible_alias = "ls")]
    List {
        /// The .deskjam file; `-` reads stdin
        file: PathBuf,
    },
}

/// Exit status when the command worked but left some files out.
const PARTIAL: u8 = 3;

/// Most `unjam` will write in total. A deskjam is untrusted input, and
/// backreferences let a few kilobytes describe terabytes of files.
const MAX_TOTAL_SIZE: u64 = 4 << 30;

/// deskjam's uploads go through vere's HTTP server, which rejects request
/// bodies over 512 MiB.
const MAX_UPLOAD_SIZE: u64 = 512 << 20;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Jam {
            dir,
            output,
            desk,
            force,
            verbose,
        } => jam(&dir, output, desk, force, verbose),
        Command::Unjam {
            file,
            output,
            force,
            verbose,
        } => unjam(&file, output, force, verbose),
        Command::List { file } => list(&file),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(PARTIAL),
        // The reader of our output went away (e.g. `jammer list x | head`).
        Err(e)
            if e.chain().any(|c| {
                c.downcast_ref::<io::Error>()
                    .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe)
            }) =>
        {
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("jammer: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn is_stdio(path: &Path) -> bool {
    path.as_os_str() == "-"
}

/// A local path, made safe to print.
fn shown(path: &Path) -> String {
    printable(path.to_string_lossy().as_bytes())
}

/// The desk name implied by a directory: its last component as the user
/// wrote it (not where a symlink leads), so `jammer jam my-app` names the
/// desk `%my-app` even if `my-app` links to `repo/desk`.
fn desk_name_for(dir: &Path) -> Result<String> {
    let absolute = std::path::absolute(dir)?;
    let mut names = Vec::new();
    for component in absolute.components() {
        match component {
            Component::Normal(name) => names.push(name),
            Component::ParentDir => {
                names.pop();
            }
            _ => {}
        }
    }
    let name = names.last().and_then(|n| n.to_str()).unwrap_or_default();
    if !pack::is_term(name) {
        bail!(
            "the directory name {:?} is not a valid desk name; pass one with --desk",
            printable(name.as_bytes())
        );
    }
    Ok(name.to_owned())
}

/// Whether `output` would land inside `dir`, where a later `|commit` of the
/// mounted desk would try (and fail) to commit it.
fn is_inside(dir: &Path, output: &Path) -> bool {
    let parent = match output.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    match (fs::canonicalize(dir), fs::canonicalize(parent)) {
        (Ok(dir), Ok(parent)) => parent.starts_with(dir),
        _ => false,
    }
}

/// Returns Ok(true) if every file the rules select was packed.
fn jam(
    dir: &Path,
    output: Option<PathBuf>,
    desk: Option<String>,
    force: bool,
    verbose: bool,
) -> Result<bool> {
    let dir_meta = fs::metadata(dir).with_context(|| format!("cannot read {}", shown(dir)))?;
    if !dir_meta.is_dir() {
        bail!("{} is not a directory", shown(dir));
    }
    let desk = match desk {
        Some(desk) if pack::is_term(&desk) => desk,
        Some(desk) => bail!(
            "{:?} is not a valid desk name (lowercase letters, digits and hyphens, starting with a letter)",
            printable(desk.as_bytes())
        ),
        None => desk_name_for(dir)?,
    };
    let explicit = output.is_some();
    let output = output.unwrap_or_else(|| PathBuf::from(format!("{desk}.deskjam")));
    if is_stdio(&output) {
        if io::stdout().is_terminal() {
            bail!("refusing to write a deskjam to the terminal; redirect stdout or pass -o FILE");
        }
    } else {
        if is_inside(dir, &output) {
            if !explicit {
                bail!(
                    "refusing to write {} inside the desk directory, where |commit would try to commit it; pass -o with a path outside it",
                    shown(&output)
                );
            }
            eprintln!(
                "warning: {} is inside the desk directory; if the desk is mounted, |commit will fail until it is removed",
                shown(&output)
            );
        }
        let is_deskjam = output.extension().is_some_and(|ext| ext == "deskjam");
        if output.exists() && !is_deskjam && !force {
            bail!(
                "{} exists and is not a .deskjam file; pass --force to replace it",
                shown(&output)
            );
        }
    }
    // Never pack a previous output of this command into the desk.
    let exclude = if is_stdio(&output) {
        None
    } else {
        fs::canonicalize(&output).ok()
    };

    let collected = pack::collect(dir, exclude.as_deref())
        .with_context(|| format!("cannot read {}", shown(dir)))?;
    let mut complete = true;
    for skipped in &collected.skipped {
        if verbose || skipped.reason.is_surprising() {
            eprintln!("skipping {}: {}", shown(&skipped.path), skipped.reason);
        }
        complete &= !skipped.reason.is_problem();
    }
    if verbose {
        for file in &collected.files {
            eprintln!("adding /{}", printable(file.path.join("/").as_bytes()));
        }
    }
    if collected.files.is_empty() {
        bail!("no files to pack in {}", shown(dir));
    }
    for (odd, count) in &collected.odd_directories {
        eprintln!(
            "note: packing {count} file(s) under {}, although that name is not a valid clay path segment (|commit does the same)",
            shown(odd)
        );
    }
    for path in pack::unparsable_config(&collected.files) {
        eprintln!(
            "warning: /{path} contains carriage returns or tabs, which clay cannot parse; deskjam would fail to commit it"
        );
    }
    let missing = pack::missing_marks(&collected.files);
    if !missing.is_empty() {
        let list: Vec<String> = missing.iter().map(|m| format!("%{m}")).collect();
        eprintln!(
            "note: no mark definition under mar/ for {}; deskjam can only import such files into a desk that has these marks",
            list.join(", ")
        );
    }
    let has_kelvin = collected.files.iter().any(|f| f.path == ["sys", "kelvin"]);
    let count = collected.files.len();
    let size: usize = collected.files.iter().map(|f| f.contents.len()).sum();
    let bytes = deskpack::encode(&desk, collected.files);

    if is_stdio(&output) {
        io::stdout()
            .lock()
            .write_all(&bytes)
            .context("cannot write to stdout")?;
    } else {
        fs::write(&output, &bytes).with_context(|| format!("cannot write {}", shown(&output)))?;
    }
    eprintln!(
        "packed {count} files ({}) from {} as %{desk} into {} ({})",
        human(size as u64),
        shown(dir),
        if is_stdio(&output) {
            "stdout".into()
        } else {
            shown(&output)
        },
        human(bytes.len() as u64),
    );
    if !has_kelvin {
        eprintln!(
            "note: there is no sys.kelvin, so deskjam can only merge these files into an existing desk, not create a new desk from them"
        );
    }
    if bytes.len() as u64 > MAX_UPLOAD_SIZE {
        eprintln!(
            "note: the file is larger than {}, the most a ship accepts in an upload; serve it over HTTP and use deskjam's import from URL instead",
            human(MAX_UPLOAD_SIZE)
        );
    }
    Ok(complete)
}

fn read_input(file: &Path) -> Result<Vec<u8>> {
    if is_stdio(file) {
        let mut bytes = Vec::new();
        io::stdin()
            .lock()
            .read_to_end(&mut bytes)
            .context("cannot read stdin")?;
        Ok(bytes)
    } else {
        fs::read(file).with_context(|| format!("cannot read {}", shown(file)))
    }
}

fn load(file: &Path) -> Result<Deskpack> {
    let bytes = read_input(file)?;
    let pack =
        deskpack::decode(&bytes).with_context(|| format!("cannot unpack {}", shown(file)))?;
    if !pack.canonical_map {
        eprintln!(
            "warning: the file table is not a canonical Hoon map; deskjam would reject this file"
        );
    }
    if pack.trailing_garbage {
        eprintln!("warning: ignoring data after the end of the jam");
    }
    Ok(pack)
}

fn clay_path(segments: &[Vec<u8>]) -> String {
    segments
        .iter()
        .map(|s| format!("/{}", printable(s)))
        .collect()
}

/// What rendering one page found, and what it cost.
struct Known {
    result: Result<usize, MarkError>,
    cost: u64,
}

/// Bounds the work of rendering a deskjam's pages by [`MAX_TOTAL_SIZE`],
/// counting both the bytes produced and the nouns walked (see
/// [`marks::Rendered`]). Pages sharing one noun, which deskjams can do at
/// no cost, are rendered once here, and failures are remembered so they
/// are not rendered again.
struct Budget {
    spent: u64,
    /// Whether successful pages will be rendered again to be written, in
    /// which case each one pays for that too.
    again: bool,
    known: HashMap<(Vec<u8>, Noun), Known>,
}

impl Budget {
    fn new(again: bool) -> Self {
        Budget {
            spent: 0,
            again,
            known: HashMap::new(),
        }
    }

    fn exhausted(&self) -> bool {
        self.spent > MAX_TOTAL_SIZE
    }

    /// The size of a page's file, charging the work it takes.
    fn size(&mut self, pack: &Deskpack, page: &Page) -> Result<usize, MarkError> {
        if let Some(declared) = marks::declared_size(&pack.arena, &page.mark, page.noun) {
            self.spent += declared.as_ref().map_or(0, |&size| size as u64);
            return declared;
        }
        let key = (page.mark.clone(), page.noun);
        if let Some(known) = self.known.get(&key) {
            if self.again && known.result.is_ok() {
                self.spent += known.cost;
            }
            return known.result.clone();
        }
        let rendered = marks::render(&pack.arena, &page.mark, page.noun);
        self.spent += rendered.cost;
        let result = rendered.result.map(|bytes| bytes.len());
        let known = Known {
            result: result.clone(),
            cost: rendered.cost,
        };
        self.known.insert(key, known);
        result
    }

    /// Why a page already failed to render, if it did.
    fn failure(&self, page: &Page) -> Option<MarkError> {
        let known = self.known.get(&(page.mark.clone(), page.noun))?;
        known.result.clone().err()
    }
}

/// Returns Ok(true) if every file was written.
fn unjam(file: &Path, output: Option<PathBuf>, force: bool, verbose: bool) -> Result<bool> {
    let pack = load(file)?;
    let desk = printable(&pack.desk);
    let root = match output {
        Some(dir) => dir,
        None if pack::is_term(&desk) => PathBuf::from(&desk),
        None => bail!("the desk name {desk:?} is not usable as a directory name; pass --output"),
    };
    // Size everything up before writing anything.
    let mut budget = Budget::new(true);
    for page in &pack.pages {
        let _ = budget.size(&pack, page);
        if budget.exhausted() {
            bail!(
                "refusing to unpack: the files add up to more than {} (counting the work of rendering them)",
                human(MAX_TOTAL_SIZE)
            );
        }
    }
    let mut writer = Writer::new(&root, force)
        .with_context(|| format!("cannot unpack into {}", shown(&root)))?;

    let mut complete = true;
    let mut seen = HashSet::new();
    let mut guessed = BTreeSet::new();
    let mut count = 0usize;
    let mut size = 0u64;
    for page in &pack.pages {
        let shown_path = clay_path(&page.path);
        let mut skip = |why: &dyn std::fmt::Display| {
            eprintln!("skipping {shown_path}: {why}");
            complete = false;
        };
        let components = match marks::unix_path(&page.path) {
            Ok(components) => components,
            Err(why) => {
                skip(&why);
                continue;
            }
        };
        if !seen.insert(components.clone()) {
            skip(&format!(
                "another file already maps to {}",
                printable(components.join("/").as_bytes())
            ));
            continue;
        }
        let rendered = match budget.failure(page) {
            Some(why) => Err(why),
            None => marks::to_bytes(&pack.arena, &page.mark, page.noun),
        };
        let contents = match rendered {
            Ok(contents) => contents,
            Err(why) => {
                skip(&why);
                continue;
            }
        };
        if !marks::is_known(&page.mark) {
            guessed.insert(printable(&page.mark));
        }
        match writer.write(&components, &contents) {
            Ok(path) => {
                if verbose {
                    eprintln!("wrote {}", shown(&path));
                }
                count += 1;
                size += contents.len() as u64;
            }
            Err(WriteError::Conflict(why)) => skip(&why),
            Err(WriteError::Io(e)) => skip(&e),
        }
    }
    if !guessed.is_empty() {
        let list: Vec<String> = guessed.iter().map(|m| format!("%{m}")).collect();
        eprintln!(
            "note: jammer does not know the marks {}; it wrote their files as the raw bytes of each page, which is right for most text and binary marks",
            list.join(", ")
        );
    }
    eprintln!(
        "unpacked {count} of {} files ({}) of %{desk} into {}",
        pack.pages.len(),
        human(size),
        shown(&root)
    );
    Ok(complete)
}

fn list(file: &Path) -> Result<bool> {
    let pack = load(file)?;
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "desk %{} ({} files)",
        printable(&pack.desk),
        pack.pages.len()
    )?;
    let mut budget = Budget::new(false);
    for page in &pack.pages {
        // Stop sizing once a hostile file has made us do too much work.
        let size = if budget.exhausted() {
            None
        } else {
            budget.size(&pack, page).ok()
        };
        let size = size.map_or_else(|| "?".to_owned(), |size| size.to_string());
        writeln!(
            out,
            "{size:>10}  %{:<10}  {}",
            printable(&page.mark),
            clay_path(&page.path)
        )?;
    }
    Ok(true)
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}
