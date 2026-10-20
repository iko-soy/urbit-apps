//! Rendering native pages, checked against output from a ship.

use jammer::jam::cue;
use jammer::marks::to_bytes;
use jammer::noun::Arena;

fn ux(text: &str) -> Vec<u8> {
    let hex: String = text
        .trim_start_matches("0x")
        .chars()
        .filter(|&c| c != '.')
        .collect();
    let hex = if hex.len() % 2 == 1 {
        format!("0{hex}")
    } else {
        hex
    };
    let mut bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    bytes.reverse();
    bytes
}

/// Render the noun `(cue jam)` with `mark`.
fn render(mark: &str, jam: &str) -> String {
    let mut arena = Arena::new();
    let noun = cue(&mut arena, &ux(jam)).unwrap().noun;
    String::from_utf8(to_bytes(&arena, mark.as_bytes(), noun).unwrap()).unwrap()
}

#[test]
fn json_matches_en_json() {
    // `(jam j)` and `(en:json:html j)` on a ship.
    let cases = [
        (
            "0x55a.21f8.e1cf.8387.82b6.946e.3e0b.5a21.d878.39b8.2cd9.382d.39a5.1b97.82b9.b29a.9718.9a03.7707.1705.6937.3f05.dfc1",
            r#"{"c":true,"a":"x","g":{},"f":[],"d":null,"b":1.5e3,"e":false}"#,
        ),
        (
            "0x7c.5e78.4127.39c4.4153.8640.fed8.cac8.40dc.401a.e412.e840.14e6.b8c4.44e3.9a00.e7c1",
            r#""q\"b\\s\n t\u0009r\u000d n del\u007f é ✓ </>""#,
        ),
        (
            "0xa9.b9f0.659c.4b74.1863.6265.746f.7571.2220.6874.6977.2079.656b.fa00.bbf8.2d49.af06.5656.4f81.cf82.c9ae.1437.7059.c3c1",
            r#"[null,0,["deep",[]],{"":"","key with \"quote\"":-1}]"#,
        ),
        (
            "0x6.e087.4328.71d1.b898.e817.7043.ca84.746e.5415.e684.746f.3417.7043.e318.71d1.b918.e817.7043.e284.746f.1415.de84.746e.f417.7043.c684.746e.3415.da84.746e.d416.9d68.4746.eb41.5edc.71d1.b818.e816.73a1.1d1b.9d05.68c7.bb83.a505.dfc1",
            r#"{"11":11,"9":9,"2":2,"4":4,"12":12,"7":7,"8":8,"6":6,"1":1,"10":10,"5":5,"3":3}"#,
        ),
    ];
    for (jam, expected) in cases {
        assert_eq!(render("json", jam), expected);
    }
}

#[test]
fn octs_marks_keep_trailing_zeros() {
    let mut arena = Arena::new();
    let size = arena.atom_u64(5);
    let data = arena.atom(&[1, 0, 2]);
    let octs = arena.cell(size, data);
    for mark in ["woff2", "wasm"] {
        assert_eq!(
            to_bytes(&arena, mark.as_bytes(), octs).unwrap(),
            [1, 0, 2, 0, 0]
        );
    }
    // the same shape wrapped in a mime
    let mite = arena.path(&["font", "woff2"]);
    let mime = arena.cell(mite, octs);
    assert_eq!(to_bytes(&arena, b"mime", mime).unwrap(), [1, 0, 2, 0, 0]);
}

#[test]
fn noun_marks_write_the_jam() {
    // `(jam [1 2 3])` on a ship, as the `%noun` mark's grow:mime writes it
    let mut arena = Arena::new();
    let noun = cue(&mut arena, &ux("0x3.4487.1dce.aded.df01"))
        .unwrap()
        .noun;
    let (_, value) = arena.as_cell(noun).unwrap();
    let bytes = to_bytes(&arena, b"noun", value).unwrap();
    let mut other = Arena::new();
    let back = cue(&mut other, &bytes).unwrap().noun;
    assert_eq!(jammer::jam::jam(&other, back), bytes);
    assert_eq!(bytes, ux("0x34.4871"));
}

#[test]
fn unknown_marks() {
    let mut arena = Arena::new();
    let text = arena.cord("hello");
    assert_eq!(to_bytes(&arena, b"custom", text).unwrap(), b"hello");
    let cell = arena.cell(text, text);
    assert!(to_bytes(&arena, b"custom", cell).is_err());
}

#[test]
fn bad_nouns_are_errors_not_panics() {
    let mut arena = Arena::new();
    let atom = arena.cord("x");
    let cell = arena.cell(atom, atom);
    for mark in [
        "mime", "txt", "kelvin", "bill", "docket-0", "json", "ship", "woff2",
    ] {
        // whichever of these shapes is wrong for the mark must be rejected cleanly
        let _ = to_bytes(&arena, mark.as_bytes(), atom);
        let _ = to_bytes(&arena, mark.as_bytes(), cell);
    }
    // a mime claiming a petabyte is refused, not allocated
    let huge = arena.atom_u64(1 << 50);
    let octs = arena.cell(huge, atom);
    let mite = arena.path(&["text", "plain"]);
    let mime = arena.cell(mite, octs);
    assert!(to_bytes(&arena, b"mime", mime).is_err());
}

/// `scot %tas` keeps NUL bytes inside kelvin labels (checked on a ship by
/// mounting `[[%1 ~] (sy ~[['Z s' 1] [0x62.0061 2] [0xff 3] [0x6100 4]])]`).
#[test]
fn kelvin_labels_keep_nul_bytes() {
    let mut arena = Arena::new();
    let wefts: Vec<_> = [
        (&b"Z s"[..], 1u64),
        (&[0x61, 0x00, 0x62][..], 2),
        (&[0xff][..], 3),
        (&[0x00, 0x61][..], 4),
    ]
    .iter()
    .map(|&(lal, num)| {
        let lal = arena.atom(lal);
        let num = arena.atom_u64(num);
        arena.cell(lal, num)
    })
    .collect();
    let set = jammer::treap::set_from_items(&mut arena, &wefts);
    let one = arena.atom_u64(1);
    let null = arena.atom(&[]);
    let tag = arena.cell(one, null);
    let waft = arena.cell(tag, set);
    assert_eq!(
        to_bytes(&arena, b"kelvin", waft).unwrap(),
        b"[%Z s 1]\n[%a\0b 2]\n[%\0a 4]\n[%\xff 3]\n"
    );
}
