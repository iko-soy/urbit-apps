//! End-to-end tests of the `jammer` binary.
//!
//! `fixtures/jfix.deskjam` was exported by deskjam from a fake ship, and
//! `fixtures/jfix/` is what that ship wrote when the same desk was mounted
//! (`|mount %jfix`), so unjam must reproduce it byte for byte.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use jammer::deskpack::{SourceFile, decode, encode};

fn jammer(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jammer"))
        .args(args)
        .output()
        .unwrap()
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// All files under `dir`, by relative path.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(d) = pending.pop() {
        for entry in fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let rel = path
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                files.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    files
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn unjam_reproduces_a_ship_mount() {
    let tmp = tempfile::tempdir().unwrap();
    let out_dir = tmp.path().join("out");
    let out = jammer(&[
        "unjam",
        fixture("jfix.deskjam").to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(tree(&out_dir), tree(&fixture("jfix")));
}

#[test]
fn jam_then_unjam_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("jfix.deskjam");
    let out = jammer(&[
        "jam",
        fixture("jfix").to_str().unwrap(),
        "-o",
        file.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let pack = decode(&fs::read(&file).unwrap()).unwrap();
    assert_eq!(pack.desk, b"jfix");
    assert!(pack.canonical_map);
    assert!(pack.pages.iter().all(|p| p.mark == b"mime"));

    let out_dir = tmp.path().join("out");
    let out = jammer(&[
        "unjam",
        file.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(tree(&out_dir), tree(&fixture("jfix")));
}

#[test]
fn default_output_names() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_jammer"))
        .args(["jam", fixture("jfix").to_str().unwrap()])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(tmp.path().join("jfix.deskjam").is_file());
    let out = Command::new(env!("CARGO_BIN_EXE_jammer"))
        .args(["unjam", "jfix.deskjam"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(tree(&tmp.path().join("jfix")), tree(&fixture("jfix")));
}

#[test]
fn list_shows_marks_and_sizes() {
    let out = jammer(&["list", fixture("jfix.deskjam").to_str().unwrap()]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("desk %jfix (25 files)\n"), "{text}");
    assert!(
        text.contains("       307  %docket-0    /desk/docket-0\n"),
        "{text}"
    );
    assert!(
        text.contains("        24  %kelvin      /sys/kelvin\n"),
        "{text}"
    );
}

#[test]
fn unjam_refuses_to_clobber_without_force() {
    let tmp = tempfile::tempdir().unwrap();
    let out_dir = tmp.path().join("out");
    fs::create_dir(&out_dir).unwrap();
    fs::write(out_dir.join("keep.txt"), "mine").unwrap();
    let file = fixture("jfix.deskjam");
    let out = jammer(&[
        "unjam",
        file.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("not empty"), "{}", stderr(&out));
    let out = jammer(&[
        "unjam",
        file.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
        "--force",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(fs::read(out_dir.join("keep.txt")).unwrap(), b"mine");
    assert!(out_dir.join("desk.docket-0").is_file());
}

#[test]
fn unjam_never_leaves_the_output_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let path = |segments: &[&str]| segments.iter().map(|s| s.to_string()).collect();
    let evil = encode(
        "evil",
        vec![
            SourceFile {
                path: path(&["..", "escape", "txt"]),
                contents: b"x".to_vec(),
            },
            SourceFile {
                path: path(&["a/b", "txt"]),
                contents: b"x".to_vec(),
            },
            SourceFile {
                path: path(&["", "txt"]),
                contents: b"x".to_vec(),
            },
            SourceFile {
                path: path(&["Upper", "txt"]),
                contents: b"x".to_vec(),
            },
            SourceFile {
                path: path(&["ok", "txt"]),
                contents: b"fine".to_vec(),
            },
        ],
    );
    let file = tmp.path().join("evil.deskjam");
    fs::write(&file, evil).unwrap();
    let out_dir = tmp.path().join("box").join("out");
    let out = jammer(&[
        "unjam",
        file.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    // Like a desk mount, `..` and the empty segment are escaped with `!`, and
    // `Upper` (not a valid @ta) is kept; only `a/b` cannot be a file name.
    assert_eq!(
        tree(&tmp.path().join("box")).keys().collect::<Vec<_>>(),
        [
            "out/!../escape.txt",
            "out/!.txt",
            "out/Upper.txt",
            "out/ok.txt"
        ]
    );
    assert!(!tmp.path().join("box/escape.txt").exists());
}

#[test]
fn unjam_does_not_write_through_symlinks() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let out_dir = tmp.path().join("out");
    fs::create_dir(&out_dir).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, out_dir.join("web")).unwrap();
    #[cfg(unix)]
    {
        let out = jammer(&[
            "unjam",
            fixture("jfix.deskjam").to_str().unwrap(),
            "-o",
            out_dir.to_str().unwrap(),
            "--force",
        ]);
        assert!(!out.status.success());
        assert!(fs::read_dir(&outside).unwrap().next().is_none());
    }
}

/// The files vere picks up on `|commit`, as observed on a ship.
#[test]
fn jam_selects_files_like_commit() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("rules");
    for sub in [".dotdir", "a.b", "sub/deep", "Upper", "emptydir"] {
        fs::create_dir_all(dir.join(sub)).unwrap();
    }
    for name in [
        "README",
        "README.md",
        ".hidden.txt",
        ".dotdir/x.txt",
        "foo.txt~",
        "foo~.txt",
        "foo.bar.txt",
        "a.b/c.txt",
        "foo.",
        "Foo.txt",
        "foo bar.txt",
        "under_score.txt",
        "sub/deep/x.txt",
        "Upper/x.txt",
        "x.TXT",
        "#emacs#.txt",
        "empty.txt",
    ] {
        fs::write(dir.join(name), if name == "empty.txt" { "" } else { "a" }).unwrap();
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("foo.bar.txt", dir.join("link.txt")).unwrap();
        std::os::unix::fs::symlink("sub", dir.join("linkdir")).unwrap();
        std::os::unix::fs::symlink("..", dir.join("sub/deep/loop")).unwrap();
        std::os::unix::fs::symlink("nowhere", dir.join("dangling.txt")).unwrap();
    }
    let file = tmp.path().join("rules.deskjam");
    let out = jammer(&["jam", dir.to_str().unwrap(), "-o", file.to_str().unwrap()]);
    // `foo.` (and on Unix the dangling link and the loop) are problems.
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    let pack = decode(&fs::read(&file).unwrap()).unwrap();
    let paths: Vec<String> = pack
        .pages
        .iter()
        .map(|p| {
            p.path
                .iter()
                .map(|s| format!("/{}", String::from_utf8_lossy(s)))
                .collect()
        })
        .collect();
    let mut expected = vec![
        "/Upper/x/txt",
        "/a.b/c/txt",
        "/empty/txt",
        "/foo.bar/txt",
        "/foo~/txt",
        "/sub/deep/x/txt",
        "/under_score/txt",
    ];
    if cfg!(unix) {
        expected.extend(["/link/txt", "/linkdir/deep/x/txt"]);
    }
    expected.sort();
    assert_eq!(paths, expected);
    let err = stderr(&out);
    assert!(err.contains("empty file extension"), "{err}");
    assert!(err.contains("not a valid clay file name"), "{err}");
    assert!(
        !err.contains("README:"),
        "files vere skips silently are only listed with -v: {err}"
    );
}

#[test]
fn jam_never_packs_its_own_output() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("desk");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("a.txt"), "a").unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_jammer"))
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap()
    };
    // By default, refuse to leave the output where |commit would find it.
    let out = run(&["jam", "."]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("inside the desk directory"),
        "{}",
        stderr(&out)
    );
    // Asked for explicitly, warn, and never pack the previous output.
    for _ in 0..2 {
        let out = run(&["jam", ".", "-o", "desk.deskjam"]);
        assert!(out.status.success(), "{}", stderr(&out));
        assert!(stderr(&out).contains("warning:"), "{}", stderr(&out));
    }
    let pack = decode(&fs::read(dir.join("desk.deskjam")).unwrap()).unwrap();
    assert_eq!(pack.pages.len(), 1);
}

#[test]
fn jam_checks_desk_names() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Bad_Name");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("a.txt"), "a").unwrap();
    let file = tmp.path().join("x.deskjam");
    let out = jammer(&["jam", dir.to_str().unwrap(), "-o", file.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("--desk"), "{}", stderr(&out));
    let out = jammer(&[
        "jam",
        dir.to_str().unwrap(),
        "-o",
        file.to_str().unwrap(),
        "-d",
        "UPPER",
    ]);
    assert!(!out.status.success());
    let out = jammer(&[
        "jam",
        dir.to_str().unwrap(),
        "-o",
        file.to_str().unwrap(),
        "-d",
        "good-name",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        decode(&fs::read(&file).unwrap()).unwrap().desk,
        b"good-name"
    );
}

#[test]
fn pipes_through_stdout_and_stdin() {
    let tmp = tempfile::tempdir().unwrap();
    let jam = jammer(&["jam", fixture("jfix").to_str().unwrap(), "-o", "-"]);
    assert!(jam.status.success(), "{}", stderr(&jam));
    let out_dir = tmp.path().join("out");
    let mut child = Command::new(env!("CARGO_BIN_EXE_jammer"))
        .args(["unjam", "-", "-o", out_dir.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&jam.stdout).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(tree(&out_dir), tree(&fixture("jfix")));
}

#[test]
fn rejects_files_that_are_not_deskjams() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("junk.deskjam");
    fs::write(&file, b"hello, world").unwrap();
    let out = jammer(&["list", file.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("cannot unpack"), "{}", stderr(&out));
    let out = jammer(&["unjam", tmp.path().to_str().unwrap()]);
    assert!(!out.status.success());
}

/// A few hundred bytes describing 20 GiB of files.
#[test]
fn unjam_refuses_decompression_bombs() {
    use jammer::jam::jam;
    use jammer::noun::Arena;
    use jammer::treap::map_from_entries;

    let mut a = Arena::new();
    let mime = a.cord("mime");
    let mite = a.path(&["text", "plain"]);
    let size = a.atom_u64(1 << 30);
    let data = a.cord("x");
    let octs = a.cell(size, data);
    let page = a.cell(mite, octs);
    let page = a.cell(mime, page);
    let entries: Vec<_> = (0..20)
        .map(|i| (a.path(&[format!("f{i}"), "txt".to_string()]), page))
        .collect();
    let map = map_from_entries(&mut a, &entries);
    let desk = a.cord("bomb");
    let tag = a.cord("deskpack-0");
    let body = a.cell(desk, map);
    let root = a.cell(tag, body);

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("bomb.deskjam");
    fs::write(&file, jam(&a, root)).unwrap();
    assert!(fs::metadata(&file).unwrap().len() < 1000);
    let out_dir = tmp.path().join("out");
    let out = jammer(&[
        "unjam",
        file.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("refusing"), "{}", stderr(&out));
    assert!(!out_dir.exists());
    let out = jammer(&["list", file.to_str().unwrap()]);
    assert!(out.status.success());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("         ?  %mime")
    );
}

/// vere escapes the segments `''`, `.`, `..` and `!...` with a leading `!` on
/// mount and strips one `!` on `|commit`; both directions were checked on a
/// ship.
#[test]
fn bang_escapes_round_trip_like_a_mount() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("bang");
    for sub in ["!..", "sub/!"] {
        fs::create_dir_all(dir.join(sub)).unwrap();
    }
    for (name, text) in [
        ("!../y.txt", "y"),
        ("sub/!/q.txt", "q"),
        ("!.txt", "e"),
        ("!!x.txt", "x"),
        ("!bang.txt", "escaped spelling"),
        ("bang.txt", "mount spelling"),
    ] {
        fs::write(dir.join(name), text).unwrap();
    }
    let file = tmp.path().join("bang.deskjam");
    let out = jammer(&["jam", dir.to_str().unwrap(), "-o", file.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("!bang.txt: it has the same clay path as"),
        "{err}"
    );
    assert!(err.contains("!!x.txt: not a valid clay file name"), "{err}");

    let pack = decode(&fs::read(&file).unwrap()).unwrap();
    let paths: Vec<Vec<String>> = pack
        .pages
        .iter()
        .map(|p| {
            p.path
                .iter()
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect()
        })
        .collect();
    let expected: Vec<Vec<&str>> = vec![
        vec!["", "txt"],
        vec!["..", "y", "txt"],
        vec!["bang", "txt"],
        vec!["sub", "", "q", "txt"],
    ];
    assert_eq!(paths, expected);

    let out_dir = tmp.path().join("out");
    let out = jammer(&[
        "unjam",
        file.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let files = tree(&out_dir);
    assert_eq!(
        files.keys().collect::<Vec<_>>(),
        ["!../y.txt", "!.txt", "bang.txt", "sub/!/q.txt"]
    );
    assert_eq!(files["bang.txt"], b"mount spelling");
}

#[test]
fn jam_notes_what_a_new_desk_would_lack() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("bare");
    fs::create_dir_all(dir.join("mar/foo")).unwrap();
    fs::write(dir.join("a.txt"), "a").unwrap();
    fs::write(dir.join("b.foo-bar"), "b").unwrap();
    fs::write(dir.join("c.hoon"), "~").unwrap();
    // %foo-bar is defined at mar/foo/bar.hoon
    fs::write(dir.join("mar/foo/bar.hoon"), "|_  a=@  --").unwrap();
    let file = tmp.path().join("bare.deskjam");
    let out = jammer(&["jam", dir.to_str().unwrap(), "-o", file.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("no mark definition under mar/ for %txt;"),
        "{err}"
    );
    assert!(err.contains("no sys.kelvin"), "{err}");

    fs::write(dir.join("mar/txt.hoon"), "|_  a=wain  --").unwrap();
    fs::create_dir(dir.join("sys")).unwrap();
    fs::write(dir.join("sys/kelvin"), "unused, no extension").unwrap();
    fs::write(dir.join("sys.kelvin"), "[%zuse 408]\n").unwrap();
    fs::write(dir.join("mar/kelvin.hoon"), "|_  a=*  --").unwrap();
    let out = jammer(&["jam", dir.to_str().unwrap(), "-o", file.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(!err.contains("note:"), "{err}");
}

#[test]
fn jam_warns_about_crlf_in_hoon_parsed_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("crlf");
    fs::create_dir_all(dir.join("mar")).unwrap();
    fs::write(dir.join("sys.kelvin"), "[%zuse 408]\r\n").unwrap();
    fs::write(dir.join("mar/kelvin.hoon"), "~").unwrap();
    // built-in conversions need no mark file; %jam is done by mar/mime.hoon
    fs::write(dir.join("a.noun"), "x").unwrap();
    fs::write(dir.join("b.mime"), "x").unwrap();
    fs::write(dir.join("c.jam"), "x").unwrap();
    fs::write(dir.join("mar/mime.hoon"), "~").unwrap();
    let file = tmp.path().join("crlf.deskjam");
    let out = jammer(&["jam", dir.to_str().unwrap(), "-o", file.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("/sys/kelvin contains carriage returns"),
        "{err}"
    );
    assert!(!err.contains("no mark definition"), "{err}");
}

#[cfg(unix)]
#[test]
fn unjam_follows_a_symlinked_output_root() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("real")).unwrap();
    std::os::unix::fs::symlink("real", tmp.path().join("link")).unwrap();
    let out = jammer(&[
        "unjam",
        fixture("jfix.deskjam").to_str().unwrap(),
        "-o",
        tmp.path().join("link").to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(tree(&tmp.path().join("real")), tree(&fixture("jfix")));
}

#[test]
fn unjam_skips_clashes_and_keeps_going() {
    let tmp = tempfile::tempdir().unwrap();
    let path = |segments: &[&str]| segments.iter().map(|s| s.to_string()).collect();
    // `/a/txt` is the file a.txt; `/a.txt/b/c` needs a directory a.txt.
    let clash = encode(
        "clash",
        vec![
            SourceFile {
                path: path(&["a", "txt"]),
                contents: b"a".to_vec(),
            },
            SourceFile {
                path: path(&["a.txt", "b", "c"]),
                contents: b"b".to_vec(),
            },
            SourceFile {
                path: path(&["z", "txt"]),
                contents: b"z".to_vec(),
            },
        ],
    );
    let file = tmp.path().join("clash.deskjam");
    fs::write(&file, clash).unwrap();
    let out_dir = tmp.path().join("out");
    let out = jammer(&[
        "unjam",
        file.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("is not a directory"),
        "{}",
        stderr(&out)
    );
    assert_eq!(
        tree(&out_dir).keys().collect::<Vec<_>>(),
        ["a.txt", "z.txt"]
    );
}

#[test]
fn jam_will_not_clobber_other_files() {
    let tmp = tempfile::tempdir().unwrap();
    let precious = tmp.path().join("notes.txt");
    fs::write(&precious, "precious").unwrap();
    let out = jammer(&[
        "jam",
        fixture("jfix").to_str().unwrap(),
        "-o",
        precious.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("--force"), "{}", stderr(&out));
    assert_eq!(fs::read(&precious).unwrap(), b"precious");
    let out = jammer(&[
        "jam",
        fixture("jfix").to_str().unwrap(),
        "-o",
        precious.to_str().unwrap(),
        "--force",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(decode(&fs::read(&precious).unwrap()).is_ok());
}

#[cfg(unix)]
#[test]
fn jam_reports_unreadable_files() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("unread");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("ok.txt"), "ok").unwrap();
    let secret = dir.join("secret.txt");
    fs::write(&secret, "secret").unwrap();
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&secret).is_ok() {
        return; // running as root
    }
    let file = tmp.path().join("u.deskjam");
    let out = jammer(&["jam", dir.to_str().unwrap(), "-o", file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    assert!(stderr(&out).contains("cannot read it"), "{}", stderr(&out));
    assert_eq!(decode(&fs::read(&file).unwrap()).unwrap().pages.len(), 1);
}

/// Names from deskjams and from the filesystem must not reach the terminal
/// raw: escape sequences could hide or forge messages.
#[test]
fn hostile_names_are_escaped() {
    use jammer::jam::jam;
    use jammer::noun::Arena;
    use jammer::treap::map_from_entries;

    let tmp = tempfile::tempdir().unwrap();
    // A page whose mark is "x<ESC>[31m" and whose noun is a cell.
    let mut a = Arena::new();
    let mark = a.atom(b"x\x1b[31m");
    let one = a.atom_u64(1);
    let noun = a.cell(one, one);
    let page = a.cell(mark, noun);
    let path = a.path(&["ok", "txt"]);
    let map = map_from_entries(&mut a, &[(path, page)]);
    let desk = a.cord("esc");
    let tag = a.cord("deskpack-0");
    let body = a.cell(desk, map);
    let root = a.cell(tag, body);
    let file = tmp.path().join("esc.deskjam");
    fs::write(&file, jam(&a, root)).unwrap();
    let out = jammer(&[
        "unjam",
        file.to_str().unwrap(),
        "-o",
        tmp.path().join("o").to_str().unwrap(),
    ]);
    assert!(!out.stderr.contains(&0x1b), "{}", stderr(&out));
    assert!(stderr(&out).contains("\\u{1b}"), "{}", stderr(&out));

    #[cfg(unix)]
    {
        let dir = tmp.path().join("evil");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("ok.txt"), "ok").unwrap();
        fs::write(dir.join("bad\x1b[2Jname.txt"), "x").unwrap();
        let out = jammer(&[
            "jam",
            dir.to_str().unwrap(),
            "-o",
            tmp.path().join("e.deskjam").to_str().unwrap(),
        ]);
        assert!(out.status.success(), "{}", stderr(&out));
        assert!(!out.stderr.contains(&0x1b), "{}", stderr(&out));
    }
}

#[cfg(unix)]
#[test]
fn desk_name_comes_from_the_path_as_given() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("repo/desk")).unwrap();
    fs::write(tmp.path().join("repo/desk/a.txt"), "a").unwrap();
    std::os::unix::fs::symlink("repo/desk", tmp.path().join("my-app")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_jammer"))
        .args(["jam", "my-app"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let pack = decode(&fs::read(tmp.path().join("my-app.deskjam")).unwrap()).unwrap();
    assert_eq!(pack.desk, b"my-app");
}

#[test]
fn many_hyphenated_extensions_are_fast() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("hy");
    fs::create_dir(&dir).unwrap();
    let ext = vec!["a"; 40].join("-");
    for i in 0..300 {
        fs::write(dir.join(format!("f{i}.{ext}")), "x").unwrap();
    }
    let start = std::time::Instant::now();
    let out = jammer(&[
        "jam",
        dir.to_str().unwrap(),
        "-o",
        tmp.path().join("hy.deskjam").to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("no mark definition"),
        "{}",
        stderr(&out)
    );
    assert!(start.elapsed().as_secs() < 10);
}

#[test]
fn unjam_notes_marks_it_guessed() {
    use jammer::jam::jam;
    use jammer::noun::Arena;
    use jammer::treap::map_from_entries;

    let mut a = Arena::new();
    let mark = a.cord("webmanifest");
    let noun = a.cord("{}");
    let page = a.cell(mark, noun);
    let path = a.path(&["site", "webmanifest"]);
    let map = map_from_entries(&mut a, &[(path, page)]);
    let desk = a.cord("guess");
    let tag = a.cord("deskpack-0");
    let body = a.cell(desk, map);
    let root = a.cell(tag, body);
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("guess.deskjam");
    fs::write(&file, jam(&a, root)).unwrap();
    let out = jammer(&[
        "unjam",
        file.to_str().unwrap(),
        "-o",
        tmp.path().join("o").to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("does not know the marks %webmanifest"),
        "{}",
        stderr(&out)
    );
    // and nothing is said for the fixture, whose marks are all known
    let out = jammer(&[
        "unjam",
        fixture("jfix.deskjam").to_str().unwrap(),
        "-o",
        tmp.path().join("f").to_str().unwrap(),
    ]);
    assert!(!stderr(&out).contains("note:"), "{}", stderr(&out));
}

/// Pages that make jammer walk a lot of nouns without producing output
/// count against the budget too, so this ~1 MB file is refused quickly.
#[test]
fn unjam_bounds_the_work_of_rendering() {
    use jammer::jam::jam;
    use jammer::noun::{Arena, Noun};
    use jammer::treap::map_from_entries;

    let mut a = Arena::new();
    // One list of 100,000 NULs; each page is a different suffix of it.
    let zero = a.atom(&[]);
    let mut suffixes = vec![zero];
    for _ in 0..100_000 {
        let next = a.cell(zero, *suffixes.last().unwrap());
        suffixes.push(next);
    }
    let tape = a.cord("tape");
    let entries: Vec<(Noun, Noun)> = (0..800)
        .map(|i| {
            let page = a.cell(tape, suffixes[100_000 - i]);
            (a.path(&[format!("f{i}"), "tape".to_string()]), page)
        })
        .collect();
    let map = map_from_entries(&mut a, &entries);
    let desk = a.cord("work");
    let tag = a.cord("deskpack-0");
    let body = a.cell(desk, map);
    let root = a.cell(tag, body);
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("work.deskjam");
    fs::write(&file, jam(&a, root)).unwrap();

    let start = std::time::Instant::now();
    let out = jammer(&[
        "unjam",
        file.to_str().unwrap(),
        "-o",
        tmp.path().join("o").to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("refusing to unpack"),
        "{}",
        stderr(&out)
    );
    let out = jammer(&["list", file.to_str().unwrap()]);
    assert!(out.status.success());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("         ?  %tape")
    );
    assert!(start.elapsed().as_secs() < 60);
}

/// A few small malformed pages are skipped one by one; the rest unpacks.
#[test]
fn unjam_skips_malformed_pages() {
    use jammer::jam::jam;
    use jammer::noun::{Arena, Noun};
    use jammer::treap::map_from_entries;

    let mut a = Arena::new();
    let txt = a.cord("txt");
    let bad = a.atom_u64(5); // a %txt page must be a list
    let line = a.cord("hello");
    let good = a.list(&[line]);
    let mut entries: Vec<(Noun, Noun)> = (0..8)
        .map(|i| {
            let page = a.cell(txt, bad);
            (a.path(&[format!("bad{i}"), "txt".to_string()]), page)
        })
        .collect();
    let page = a.cell(txt, good);
    entries.push((a.path(&["good", "txt"]), page));
    let map = map_from_entries(&mut a, &entries);
    let desk = a.cord("mixed");
    let tag = a.cord("deskpack-0");
    let body = a.cell(desk, map);
    let root = a.cell(tag, body);
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("mixed.deskjam");
    fs::write(&file, jam(&a, root)).unwrap();
    let out_dir = tmp.path().join("o");
    let out = jammer(&[
        "unjam",
        file.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    assert_eq!(
        stderr(&out).matches("not a valid %txt noun").count(),
        8,
        "{}",
        stderr(&out)
    );
    assert_eq!(
        tree(&out_dir).get("good.txt").map(Vec::as_slice),
        Some(&b"hello"[..])
    );
}
