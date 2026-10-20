//! Vectors from `(scow %p ...)`, `(scow %ud ...)` etc. on a ship.

use jammer::aura::{p, ud, uv, ux};

fn le(hex: &str) -> Vec<u8> {
    let hex = if hex.len() % 2 == 1 {
        format!("0{hex}")
    } else {
        hex.to_string()
    };
    let mut bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    bytes.reverse();
    bytes
}

#[test]
fn ships() {
    for (hex, name) in [
        ("0", "~zod"),
        ("1", "~nec"),
        ("ff", "~fes"),
        ("100", "~marzod"),
        ("101", "~marnec"),
        ("ffff", "~fipfes"),
        ("10000", "~dapnep-ronmyl"),
        ("10001", "~milrys-soglec"),
        ("7f123456", "~watrup-worrun"),
        ("ffffffff", "~dostec-risfen"),
        ("100000000", "~doznec-dozzod-dozzod"),
        ("7e710000deadbeef", "~mister-dozzod-biltyc-mitfyl"),
        ("7e71817100000000", "~mister-dister-dozzod-dozzod"),
        ("ffffffffffffffff", "~fipfes-fipfes-dostec-risfen"),
        ("10000000000000000", "~doznec--dozzod-dozzod-dozzod-dozzod"),
        (
            "deadbeefcafebabe123456789abcdef0",
            "~ribmug-hilmun-masnev-lodlev--doprut-posfel-tilbyt-riblyr",
        ),
        ("123456", "~figsut-fanrut"),
    ] {
        assert_eq!(p(&le(hex)), name, "@p of 0x{hex}");
    }
}

#[test]
fn numbers() {
    for (hex, d, x, v) in [
        ("0", "0", "0x0", "0v0"),
        ("1", "1", "0x1", "0v1"),
        ("9", "9", "0x9", "0v9"),
        ("a", "10", "0xa", "0va"),
        ("1f", "31", "0x1f", "0vv"),
        ("20", "32", "0x20", "0v10"),
        ("3e7", "999", "0x3e7", "0vv7"),
        ("3e8", "1.000", "0x3e8", "0vv8"),
        ("400", "1.024", "0x400", "0v100"),
        ("75bcd15", "123.456.789", "0x75b.cd15", "0v3.lnj8l"),
        (
            "10000000000000000",
            "18.446.744.073.709.551.616",
            "0x1.0000.0000.0000.0000",
            "0vg00.00000.00000",
        ),
        (
            "10000000000000000000000000",
            "1.267.650.600.228.229.401.496.703.205.376",
            "0x10.0000.0000.0000.0000.0000.0000",
            "0v1.00000.00000.00000.00000",
        ),
    ] {
        let atom = le(hex);
        assert_eq!(ud(&atom), d);
        assert_eq!(ux(&atom), x);
        assert_eq!(uv(&atom), v);
    }
}
