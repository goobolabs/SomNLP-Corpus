//! Conservative text normalization and the digests used for deduplication.
//!
//! Somali is written in the Latin script, so normalization keeps every letter,
//! apostrophe (glottal stop), diacritic and capital exactly as mined: nothing
//! is lowercased and no spelling is corrected. Only invisible characters and
//! redundant whitespace are touched.

use blake2::digest::consts::U16;
use blake2::{Blake2b, Digest};
use unicode_normalization::UnicodeNormalization;

/// BLAKE2b truncated to 16 bytes — plenty for in-memory dedup keys.
type Blake2b128 = Blake2b<U16>;

/// Zero-width and formatting characters removed outright.
const ZERO_WIDTH: [char; 5] = ['\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{FEFF}'];

/// Non-breaking spaces mapped to a plain space before whitespace collapsing.
const NON_BREAKING_SPACES: [char; 3] = ['\u{00A0}', '\u{202F}', '\u{2007}'];

/// NFC-compose, drop zero-width characters, turn non-breaking spaces into
/// plain ones, collapse whitespace runs (including line breaks) to one space,
/// and trim.
pub fn normalize_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    collapse(text.nfc().filter_map(|ch| {
        if ZERO_WIDTH.contains(&ch) {
            None
        } else if NON_BREAKING_SPACES.contains(&ch) {
            Some(' ')
        } else {
            Some(ch)
        }
    }))
}

/// Canonical form used for dedup hashing: NFC plus collapsed whitespace.
///
/// Deliberately weaker than [`normalize_text`] — it decides whether two rows
/// are the same pair, so it must not change what the row *says*.
pub fn canonical_text(text: &str) -> String {
    collapse(text.nfc())
}

/// Digest of a canonicalized English/Somali pair.
pub fn pair_digest(english: &str, somali: &str) -> [u8; 16] {
    let mut hasher = Blake2b128::new();
    hasher.update(canonical_text(english).as_bytes());
    hasher.update(b"\0");
    hasher.update(canonical_text(somali).as_bytes());
    hasher.finalize().into()
}

/// Digest of a canonicalized Somali sentence.
pub fn text_digest(text: &str) -> [u8; 16] {
    Blake2b128::digest(canonical_text(text).as_bytes()).into()
}

/// Collapse every whitespace run to a single space and trim both ends.
fn collapse(chars: impl Iterator<Item = char>) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for ch in chars {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_decomposed_forms() {
        assert_eq!(normalize_text("cafe\u{0301}"), "caf\u{00E9}");
    }

    #[test]
    fn removes_zero_width_characters() {
        assert_eq!(normalize_text("a\u{200B}b\u{200C}c\u{FEFF}"), "abc");
    }

    #[test]
    fn collapses_non_breaking_and_repeated_whitespace() {
        assert_eq!(normalize_text("a\u{00A0}\u{202F}  b"), "a b");
    }

    #[test]
    fn trims_and_flattens_line_endings() {
        assert_eq!(normalize_text("  x\r\ny  "), "x y");
    }

    #[test]
    fn preserves_case_apostrophes_and_somali_letters() {
        assert_eq!(normalize_text("Waa Cali'ga"), "Waa Cali'ga");
        assert_eq!(
            normalize_text("Ma'aan arag shaqaale'da Soomaaliyeed"),
            "Ma'aan arag shaqaale'da Soomaaliyeed"
        );
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(normalize_text(""), "");
        assert_eq!(normalize_text("   \n\t "), "");
    }

    #[test]
    fn pair_digest_ignores_whitespace_differences() {
        assert_eq!(
            pair_digest("hello  world", "salaan"),
            pair_digest("hello world", "salaan")
        );
        assert_eq!(pair_digest("a", "b").len(), 16);
    }

    #[test]
    fn pair_digest_separates_the_two_sides() {
        // Without the separator "ab" + "" and "a" + "b" would collide.
        assert_ne!(pair_digest("ab", ""), pair_digest("a", "b"));
    }

    #[test]
    fn text_digest_matches_only_canonically_equal_text() {
        assert_eq!(text_digest(" isku  mid "), text_digest("isku mid"));
        assert_ne!(text_digest("isku mid"), text_digest("Isku mid"));
    }
}
