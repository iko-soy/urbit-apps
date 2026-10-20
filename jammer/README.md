# jammer

Pack a desk directory into a `.deskjam` file that you can upload to
[deskjam](https://github.com/tinnus-napbus/deskjam), and unpack `.deskjam`
files back into directories, without a ship.

You can install the files jammer makes with
[iko-soy/deskjam](https://github.com/iko-soy/deskjam), a fork of deskjam that
installs and updates apps from them: upload one at
`https://your-ship/apps/deskjam`. See
[Installing with deskjam](../README.md#installing-with-deskjam) for the steps.

```console
$ jammer jam ./desk -d my-app
packed 42 files (100.7 KiB) from ./desk as %my-app into my-app.deskjam (101.8 KiB)

$ jammer list my-app.deskjam
desk %my-app (42 files)
      6940  %mime        /app/my-app/hoon
        16  %mime        /desk/bill
...

$ jammer unjam garden.deskjam          # writes ./garden/
unpacked 166 of 166 files (421.6 KiB) of %garden into garden
```

## Install

```console
cargo install --path .
```

## Commands

- `jammer jam <DIR> [-o FILE] [-d DESK] [-f] [-v]` packs a desk directory,
  such as a mounted desk or a repository's `desk/` folder.
  - The desk name defaults to the directory's name, as you wrote it.
  - The output defaults to `./<desk>.deskjam`. Use `-o -` to write to stdout.
  - It will not leave its output inside the desk directory, because a later
    `|commit` of a mounted desk would trip over it.
  - It replaces an existing file only if that is a `.deskjam`, or with
    `--force`.
- `jammer unjam <FILE> [-o DIR] [--force]` unpacks into `./<desk>` by
  default. It refuses to write into a non-empty directory unless you pass
  `--force`. Use `-` as the file to read from stdin.
- `jammer list <FILE>` (or `ls`) prints the desk name and each file's size,
  mark and path.

## What you get

**`jam` builds the same desk `|commit` would.**

- Files are picked by the same rules vere uses when you `|commit` a mounted
  desk:
  - Hidden files and directories are skipped.
  - A file needs an extension, must not end in `~`, and its name may only
    contain `a-z 0-9 - . _ ~`.
  - Symlinks are followed.
  - `dir/name.ext` becomes `/dir/name/ext`.
  - A leading `!` is dropped from each name, undoing the escape a mount uses
    for segments like `..`. If two names map to the same path, the one a
    mount would write wins.
- Every file is stored as a `%mime` page, just as vere sends them on
  `|commit`. When deskjam commits the desk, clay converts each page to the
  mark named by its extension, using the marks in the target desk.
- A new desk therefore needs `sys.kelvin` and a `mar/` file for every
  extension other than `.hoon`. jammer tells you when either is missing.
  Merging into an existing desk only needs that desk to have the marks.

**`unjam` writes what a desk mount writes.**

- Desks exported by deskjam contain clay's native pages. jammer renders them
  the way each mark's `grow:mime` arm does for `|mount`:
  - `hoon`, `js`, `css`, `html`, `svg`, `png`, `jam` and other atom marks:
    the raw bytes.
  - `txt`, `md` and `pem`: the lines joined.
  - `json`: printed exactly like `en:json:html`, including its key order.
  - `kelvin`, `bill`, `docket-0`, `ship` and `story`: the same text the
    marks print.
  - `wasm` and `woff2`: the bytes of the `octs`.
  - `noun`, `pill` and similar: the jam of the noun.
  - `tape`: the characters.
  - `mime`: the raw file contents.
- Marks jammer cannot render are reported and skipped. These are XML
  (`hymn`, `urb`), `eth-txs`, and marks a mount cannot write either.
- Marks are rendered by name, as `%base` and landscape define them. If a
  desk redefines one of these marks in its own `mar/`, its mount may differ.
  Pages with marks jammer has never heard of are written as the raw bytes of
  their atom, which suits most text and binary marks, and jammer says which
  marks it guessed.
- Untrusted input is handled safely:
  - Segments like `..` are written as `!..`, as a mount does, so paths never
    escape the output directory.
  - Nothing is written through symlinks, and on Unix every step of a path
    is taken relative to an already-verified directory, so this cannot be
    raced.
  - Backreferences let a tiny jam describe enormous files, so each file is
    capped at 1 GiB. The whole unpack is capped at the equivalent of 4 GiB,
    counting the bytes produced and the nouns walked to produce them. This
    is checked before anything is written, and at most one file is held in
    memory at a time.

## Exit status

- `0`: everything was packed or unpacked.
- `1`: an error stopped the command.
- `3`: it finished, but left some files out. jam does this for files it
  could not read; unjam for files it could not render or write. The skipped
  files are listed on stderr.

## How it was checked

- **Serialization:** jam, cue, `mug` and Hoon map construction are
  byte-identical to a ship's, checked with vectors from `urbit eval`.
- **Real files:** deskjam files exported by real ships re-encode to the
  identical bytes.
- **On a fake ship running deskjam:**
  - Directories packed by jammer and uploaded through deskjam's web UI
    produced desks identical to the originals, file for file. The new desks'
    clay pages were also identical to those a `|commit` of the same
    directory produced.
  - `unjam` output for `%base`, `%groups`, `%landscape`, `%webterm` and
    `%deskjam` matched the ship's own `|mount` of those desks byte for byte.
- **Regression tests:**
  - `tests/fixtures/` holds such an export together with the ship's mount of
    it.
  - `tests/vectors/` holds about 1,500 outputs of the real marks and of
    `scow`, generated with `urbit eval`.

```console
cargo test
```
