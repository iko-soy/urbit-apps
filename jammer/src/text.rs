//! Printing untrusted names (from deskjam files or the filesystem).

/// `bytes` as text that is safe to print to a terminal: control characters
/// and invisible formatting characters (bidi overrides, zero-width spaces)
/// are escaped, so hostile names cannot rewrite or disguise output.
pub fn printable(bytes: &[u8]) -> String {
    let mut out = String::new();
    for c in String::from_utf8_lossy(bytes).chars() {
        if c.is_control() || is_format(c) {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}

/// Unicode format characters (general category Cf) that change how text is
/// displayed without being visible themselves.
fn is_format(c: char) -> bool {
    matches!(
        c,
        '\u{00ad}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}

#[cfg(test)]
mod tests {
    use super::printable;

    #[test]
    fn escapes_terminal_tricks() {
        assert_eq!(printable(b"ok/name.txt"), "ok/name.txt");
        assert_eq!(printable(b"x\x1b[31m"), "x\\u{1b}[31m");
        assert_eq!(printable("a\u{202e}b".as_bytes()), "a\\u{202e}b");
        assert_eq!(printable(&[0x61, 0xff]), "a\u{fffd}");
    }
}
