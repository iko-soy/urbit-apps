use jammer::deskpack::{SourceFile, decode, encode};

fn file(path: &[&str], contents: &[u8]) -> SourceFile {
    SourceFile {
        path: path.iter().map(|s| s.to_string()).collect(),
        contents: contents.to_vec(),
    }
}

/// `(jam [%deskpack-0 %foo (~(gas by *(map path page:clay)) files)])` on a ship.
const SHIP_JAM: &str = "a802e1d31930ded10db63634b17c075b9b2b27c05580004a05c31930ded10db3b7383c07734b13c07134b63c055c6c4c2f03431930ded10db737b7b47c070f1bc1c185e02b34ba70606840cae6eaf44ab6e4030836e69616c703c03d1e195d3e02e56d696df80db9a5d9b195adf00f37973780aef6f66780e05ad6c6c2e0d6e6cac87401";

fn ship_bytes() -> Vec<u8> {
    let mut bytes: Vec<u8> = (0..SHIP_JAM.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&SHIP_JAM[i..i + 2], 16).unwrap())
        .collect();
    bytes.reverse();
    bytes
}

fn sample() -> Vec<SourceFile> {
    vec![
        file(&["app", "foo", "hoon"], b"abc"),
        file(&["sys", "kelvin"], b"[%zuse 408]\n"),
        file(&["desk", "bill"], b""),
        file(&["lib", "bin", "png"], &[1, 0, 2, 0, 0]),
    ]
}

#[test]
fn encode_matches_ship() {
    assert_eq!(encode("foo", sample()), ship_bytes());
}

#[test]
fn encode_ignores_file_order() {
    let mut files = sample();
    files.reverse();
    assert_eq!(encode("foo", files), ship_bytes());
}

#[test]
fn decode_round_trip() {
    let pack = decode(&ship_bytes()).unwrap();
    assert_eq!(pack.desk, b"foo");
    assert!(pack.canonical_map);
    assert!(!pack.trailing_garbage);
    let paths: Vec<String> = pack
        .pages
        .iter()
        .map(|p| {
            p.path
                .iter()
                .map(|s| String::from_utf8(s.clone()).unwrap())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect();
    assert_eq!(
        paths,
        ["app/foo/hoon", "desk/bill", "lib/bin/png", "sys/kelvin"]
    );
    assert!(pack.pages.iter().all(|p| p.mark == b"mime"));
}

#[test]
fn decode_rejects_other_jams() {
    // (jam [%foo ~])
    assert!(decode(&[0x02]).is_err());
    assert!(decode(&[]).is_err());
}
