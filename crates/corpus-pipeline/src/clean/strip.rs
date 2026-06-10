//! Removal of control and invisible characters that NFC leaves behind and that
//! poison shingling and hashing. See docs/CLEANING_PLAN.md (Clean stage).

/// Keep a character: drop control characters (except newline and tab) and the
/// common invisible web-junk code points.
fn keep(c: char) -> bool {
    match c {
        '\n' | '\t' => true,
        // Zero-width, BOM/ZWNBSP, soft hyphen.
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{00AD}' => false,
        // Bidirectional controls.
        '\u{202A}'..='\u{202E}' => false,
        c if c.is_control() => false,
        _ => true,
    }
}

/// Strip control and invisible characters.
pub fn strip_invisibles(text: &str) -> String {
    text.chars().filter(|&c| keep(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_newline_and_tab() {
        assert_eq!(strip_invisibles("a\nb\tc"), "a\nb\tc");
    }

    #[test]
    fn removes_zero_width_and_bom() {
        assert_eq!(strip_invisibles("a\u{200B}b\u{FEFF}c"), "abc");
    }

    #[test]
    fn removes_carriage_return_and_controls() {
        assert_eq!(strip_invisibles("a\r\nb\u{0007}c"), "a\nbc");
    }

    #[test]
    fn removes_soft_hyphen_and_bidi() {
        assert_eq!(strip_invisibles("a\u{00AD}b\u{202E}c"), "abc");
    }
}
