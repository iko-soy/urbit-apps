//! Vectors produced by a real ship (`urbit eval`, vere 4.6, zuse 408).

use jammer::jam::{cue, jam};
use jammer::noun::{Arena, Noun};
use jammer::treap::{is_valid_treap, map_from_entries, set_from_items, tree_items};

/// Parse Hoon `@ux` syntax (`0x2.4ee1`) into little-endian bytes.
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
    while bytes.last() == Some(&0) {
        bytes.pop();
    }
    bytes
}

fn num(a: &mut Arena, v: u64) -> Noun {
    a.atom_u64(v)
}

fn check_jam(a: &mut Arena, noun: Noun, expected: &str) {
    let bytes = jam(a, noun);
    assert_eq!(bytes, ux(expected), "jam mismatch for expected {expected}");
    let cued = cue(a, &bytes).unwrap();
    assert_eq!(cued.noun, noun, "cue did not round-trip for {expected}");
    assert!(!cued.trailing_garbage);
}

#[test]
fn jam_matches_ship() {
    let mut a = Arena::new();
    let n = num(&mut a, 0);
    check_jam(&mut a, n, "0x2");
    let n = num(&mut a, 1);
    check_jam(&mut a, n, "0xc");
    let one = num(&mut a, 1);
    let n = a.cell(one, one);
    check_jam(&mut a, n, "0x331");
    let (z, nineteen) = (num(&mut a, 0), num(&mut a, 19));
    let n = a.cell(z, nineteen);
    check_jam(&mut a, n, "0x9b09");
    let five = num(&mut a, 5);
    let n = a.cell(five, five);
    check_jam(&mut a, n, "0x2.4ee1");
    let mut big = vec![0u8; 8];
    big.push(1);
    let n = a.atom(&big);
    check_jam(&mut a, n, "0x8000.0000.0000.0000.0300");
    let n = a.cord("Hello, world!");
    check_jam(&mut a, n, "0x10.b236.3937.bb90.1637.b636.32a4.4d00");
    let two = num(&mut a, 2);
    let p = a.cell(one, two);
    let n = a.cell(p, p);
    check_jam(&mut a, n, "0x49.c8c5");
    let n = a.path(&["app", "foo", "hoon"]);
    check_jam(&mut a, n, "0x2d.cded.ed1f.01de.decc.f01e.0e0c.2f01");
    let mut bex1000 = vec![0u8; 125];
    bex1000.push(1);
    let n = a.atom(&bex1000);
    check_jam(
        &mut a,
        n,
        "0x2000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.0000.001e.9800",
    );
    let large = num(&mut a, 12_345_678_901_234_567_890);
    let n = a.cell(large, large);
    check_jam(&mut a, n, "0x127.56a9.5319.d63e.15a4.0401");
    let b = num(&mut a, 256);
    let bb = a.cell(b, b);
    let n = a.cell(b, bb);
    check_jam(&mut a, n, "0x24.e4d8.0181");
    let mime = a.cord("mime");
    let mite = a.path(&["text", "plain"]);
    let (three, abc) = (num(&mut a, 3), a.cord("abc"));
    let octs = a.cell(three, abc);
    let m = a.cell(mite, octs);
    let n = a.cell(mime, m);
    check_jam(
        &mut a,
        n,
        "0x18d8.985e.0686.dcd2.c2d8.e078.07a3.c32b.a7c0.5cad.ad2d.bf01",
    );
}

#[test]
fn mug_matches_ship() {
    let mut a = Arena::new();
    let zero = num(&mut a, 0);
    let one = num(&mut a, 1);
    let two = num(&mut a, 2);
    let letter = a.cord("a");
    let zz = a.cell(zero, zero);
    let one_two = a.cell(one, two);
    let path = a.path(&["app", "foo", "hoon"]);
    let hello = a.cord("Hello, world!");
    for (noun, expected) in [
        (zero, 0x79ff_04e8),
        (one, 0x715c_2a60),
        (letter, 0x36b6_f52a),
        (zz, 0x192f_5588),
        (one_two, 0x6a36_c1d9),
        (path, 0x5747_c2a6),
        (hello, 0x4d44_1035),
    ] {
        assert_eq!(a.mug(noun), expected);
    }
}

#[test]
fn maps_and_sets_match_ship() {
    let mut a = Arena::new();
    let entries: Vec<(Noun, Noun)> = [
        (&["a", "hoon"][..], 1),
        (&["b", "txt"][..], 2),
        (&["sys", "kelvin"][..], 3),
    ]
    .iter()
    .map(|(p, v)| (a.path(p), a.atom_u64(*v)))
    .collect();
    let map = map_from_entries(&mut a, &entries);
    assert!(is_valid_treap(&a, map, |item| a
        .as_cell(item)
        .map(|(k, _)| k)));
    assert_eq!(
        jam(&a, map),
        ux(
            "0xb210.f2dc.dede.d1f0.1c3c.15a6.45d1.e1d1.e038.b82a.e8b7.34bb.3632.b5be.01e6.f2e6.f015"
        )
    );

    let zuse = a.cord("zuse");
    let wefts: Vec<Noun> = [409, 408, 410]
        .iter()
        .map(|&v| {
            let n = a.atom_u64(v);
            a.cell(zuse, n)
        })
        .collect();
    let set = set_from_items(&mut a, &wefts);
    assert_eq!(
        jam(&a, set),
        ux("0xa9c.d182.66ae.64c1.3359.cc18.32b9.babd.7c05")
    );

    let entries: Vec<(Noun, Noun)> = (1..=40u64)
        .map(|i| (a.path(&["lib", &i.to_string(), "hoon"]), a.atom_u64(i)))
        .collect();
    let map = map_from_entries(&mut a, &entries);
    assert!(is_valid_treap(&a, map, |item| a
        .as_cell(item)
        .map(|(k, _)| k)));
    assert_eq!(tree_items(&a, map).unwrap().len(), 40);
    assert_eq!(
        jam(&a, map),
        ux(
            "0x1410.c3f3.083c.7819.e81d.9ab3.e420.f1ea.63a0.766a.c10c.3fb0.83c7.899e.81d9.ab33.5083.c7c1.9e81.d9aa.f108.3c7a.18e8.1d9a.b043.0e44.1e3c.a83b.3566.6c20.f1f2.63a0.766a.c10c.3830.83c7.b18e.81d9.aac1.083c.7c50.766a.b242.0f1f.341d.9aac.b420.f1ee.67a0.766a.c10c.3d21.078f.331d.03b3.56a0.861e.883c.79d0.766a.b050.83c7.919e.81d9.ab04.30e2.a107.8f43.3d03.b355.f883.c7bd.0766.ab3d.083c.7c99.e81d.9ab4.10c3.c308.3c7a.1968.1d9a.acc4.1e3d.283b.3560.861c.7420.f1ea.67a0.766a.bd61.078f.932d.03b3.558b.083c.7b98.e81d.9ab3.6420.f1e2.63a0.766a.b842.0f1e.463a.0766.ab56.1078.f132.d03b.3560.861e.c41e.3da8.3b35.5b30.83c7.9196.81d9.ab41.0c3c.b083.c7a9.9681.d9aa.e984.1e3d.8cb4.0ecd.5821.8776.1078.f332.d03b.3560.861c.9841.e3e0.c740.ecd5.9c83.c78d.0766.ac10.c3a2.841e.3c0d.340e.cd56.4a10.78f6.33d0.3b35.5e30.83c7.c196.81d9.ab30.d083.c799.9e81.d9aa.dc41.e3d6.83b3.569a.2107.8f03.1d03.b355.db08.3c7b.9968.1d9a.ad18.5b9b.dbda.3e03.8196.81c4.d2d8.f015"
        )
    );
}

#[test]
fn cue_is_as_lenient_as_hoon() {
    let mut a = Arena::new();
    // (cue 0x2d0) => 5: oversized length field
    let five = a.atom_u64(5);
    assert_eq!(cue(&mut a, &ux("0x2d0")).unwrap().noun, five);
    // (cue 0xc23b.86e1) => [5 5 5]: backref to a second occurrence
    let tail = a.cell(five, five);
    let triple = a.cell(five, tail);
    assert_eq!(cue(&mut a, &ux("0xc23b.86e1")).unwrap().noun, triple);
    // (cue 0x17b8) => 5: trailing bits are ignored
    let cued = cue(&mut a, &ux("0x17b8")).unwrap();
    assert_eq!(cued.noun, five);
    assert!(cued.trailing_garbage);
    // (cue 0x7.f800.0040.0000) => 255: payload bits past the end read as 0
    let n255 = a.atom_u64(255);
    assert_eq!(cue(&mut a, &ux("0x7.f800.0040.0000")).unwrap().noun, n255);
}

#[test]
fn cue_rejects_garbage() {
    let mut a = Arena::new();
    assert!(cue(&mut a, &[]).is_err());
    // a lone cell tag
    assert!(cue(&mut a, &[0b01]).is_err());
    // [0 <backref to bit 1>]: cell `1 0`, atom 0 `0 1`, backref `1 1` + mat(1)
    // `0 1 1`. No noun starts at bit 1.
    assert!(matches!(
        cue(&mut a, &[0xb9, 0x01]),
        Err(jammer::jam::CueError::BadBackref { target: 1, .. })
    ));
    // the same stream pointing at bit 2, where the atom 0 starts, is fine:
    // backref `1 1` + mat(2) `0 0 1 0 0 1`... built with jam itself instead
    let zero = num(&mut a, 0);
    let pair = a.cell(zero, zero);
    let bytes = jam(&a, pair);
    assert_eq!(cue(&mut a, &bytes).unwrap().noun, pair);
    // a 65-zero length prefix
    let mut bytes = vec![0u8; 9];
    bytes.push(0x80);
    assert!(cue(&mut a, &bytes).is_err());
    // a huge declared atom is not allocated
    let mut huge = vec![0u8; 7];
    huge.push(0x80); // 63 zeros then a 1 at bit 63: width >= 2^62
    huge.extend_from_slice(&[0xff; 8]);
    let _ = cue(&mut a, &huge);
}

/// Nouns whose first murmur3 hash folds to zero, so `mug` retries with the
/// next seed (found by inverting murmur3; values from a ship).
#[test]
fn mug_retries_with_next_seed() {
    let mut a = Arena::new();
    let one = num(&mut a, 1);
    let x = num(&mut a, 0x6ddb_5cd6);
    let y = num(&mut a, 0xb33b_0b40);
    let p = num(&mut a, 0xb91a_4685);
    let p = a.cell(one, p);
    let q = num(&mut a, 0x709a_4eed);
    let q = a.cell(one, q);
    assert_eq!(a.mug(x), 0x0429_ceea);
    assert_eq!(a.mug(y), 0x5cbf_3de5);
    assert_eq!(a.mug(p), 0x5381_734e);
    assert_eq!(a.mug(q), 0x7bd5_70ca);
    for (noun, double) in [(x, 0x7c30_923f), (y, 0x56af_cb80), (p, 0x5540_af45)] {
        let mug = u64::from(a.mug(noun));
        let mug = num(&mut a, mug);
        assert_eq!(a.mug(mug), double);
    }
}

#[test]
fn cue_bounds_the_number_of_nouns() {
    // A list of n nulls is ~4 bits per element and decodes to 2n nouns.
    let mut a = Arena::new();
    let zero = num(&mut a, 0);
    let mut list = zero;
    for _ in 0..1000 {
        list = a.cell(zero, list);
    }
    let bytes = jam(&a, list);
    let mut b = Arena::new();
    assert!(matches!(
        jammer::jam::cue_with_limit(&mut b, &bytes, 2000),
        Err(jammer::jam::CueError::TooLarge { .. })
    ));
    assert_eq!(
        jammer::jam::cue_with_limit(&mut b, &bytes, 2001)
            .unwrap()
            .noun,
        {
            let mut c = Arena::new();
            let n = cue(&mut c, &bytes).unwrap().noun;
            // same arena handles differ; compare by re-jamming
            assert_eq!(jam(&c, n), bytes);
            cue(&mut b, &bytes).unwrap().noun
        }
    );
}

/// vere's cue gives up on a length prefix of 64 zero bits (%meme), so a
/// deskjam containing one could never be imported; don't accept it either.
#[test]
fn cue_rejects_64_bit_length_prefixes() {
    let mut bytes = vec![0u8; 8];
    bytes.push(0x02); // bit 64 = 0, bit 65 = 1: tag 0, then 64 zeros, then 1
    bytes.extend([0u8; 8]);
    let mut a = Arena::new();
    assert!(matches!(
        cue(&mut a, &bytes),
        Err(jammer::jam::CueError::TooLarge { .. })
    ));
}
