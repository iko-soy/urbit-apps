//! Table-driven checks against output from a ship (`tests/vectors/`).

use std::fs;
use std::path::Path;

use jammer::aura;
use jammer::jam::cue;
use jammer::marks::{MarkError, to_bytes};
use jammer::noun::Arena;

fn table(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/vectors")
            .join(name),
    )
    .unwrap()
}

/// Hex digits in file order to bytes.
fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// A Hoon atom literal (`0x1.abcd` or `123.456`) as little-endian bytes.
fn literal(text: &str) -> Vec<u8> {
    if let Some(digits) = text.strip_prefix("0x") {
        let digits: String = digits.chars().filter(|&c| c != '.').collect();
        let digits = if digits.len() % 2 == 1 {
            format!("0{digits}")
        } else {
            digits
        };
        let mut bytes = hex(&digits);
        bytes.reverse();
        bytes
    } else {
        let digits: String = text.chars().filter(|&c| c != '.').collect();
        digits.parse::<u128>().unwrap().to_le_bytes().to_vec()
    }
}

#[test]
fn marks_render_like_grow_mime() {
    let mut checked = 0;
    for line in table("marks.tsv").lines().filter(|l| !l.starts_with('#')) {
        let fields: Vec<&str> = line.split('\t').collect();
        let (mark, sample, jam, file) = (fields[0], fields[1], fields[2], fields[3]);
        let mut arena = Arena::new();
        let noun = cue(&mut arena, &hex(jam)).unwrap().noun;
        let rendered = to_bytes(&arena, mark.as_bytes(), noun);
        match mark {
            // printers jammer does not implement must say so, not guess
            "hymn" | "urb" | "snip" | "eth-txs" => {
                assert!(
                    matches!(rendered, Err(MarkError::Unsupported(_))),
                    "%{mark} {sample}"
                )
            }
            _ => {
                assert_eq!(rendered.unwrap(), hex(file), "%{mark} {sample}");
                checked += 1;
            }
        }
    }
    assert!(checked > 200);
}

#[test]
fn dockets_render_like_spit_docket() {
    let mut checked = 0;
    for line in table("dockets.txt").lines().filter(|l| !l.starts_with('#')) {
        let fields: Vec<&str> = line.split('|').map(str::trim).collect();
        let (name, jam, file) = (fields[0], fields[2], fields[3]);
        let mut arena = Arena::new();
        let noun = cue(&mut arena, &hex(jam)).unwrap().noun;
        assert_eq!(
            to_bytes(&arena, b"docket-0", noun).unwrap(),
            hex(file),
            "{name}"
        );
        checked += 1;
    }
    assert!(checked > 100);
}

#[test]
fn auras_print_like_scow() {
    let mut checked = 0;
    for line in table("auras.txt").lines().filter(|l| !l.starts_with('#')) {
        let Some((expr, expected)) = line.split_once(" => ") else {
            continue;
        };
        let expected = expected.trim().trim_matches('"');
        let inner = expr
            .trim()
            .trim_start_matches("(scow %")
            .trim_end_matches(')');
        let (which, arg) = inner.split_once(' ').unwrap();
        let atom = literal(arg);
        let printed = match which {
            "p" => aura::p(&atom),
            "ud" => aura::ud(&atom),
            "ux" => aura::ux(&atom),
            "uv" => aura::uv(&atom),
            other => panic!("unknown aura {other}"),
        };
        assert_eq!(printed, expected, "{expr}");
        checked += 1;
    }
    assert!(checked > 1000);
}
