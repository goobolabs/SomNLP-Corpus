//! Text normalization used for exact-dedup hashing and stable IDs.
//!
//! This form is for hashing only. It does not change the text stored in records;
//! see docs/ID_STRATEGY.md.

use unicode_normalization::UnicodeNormalization;

/// Canonical normalization for content hashing: NFC, trim, collapse internal
/// whitespace to single spaces, lowercase.
///
/// Keep this minimal. Anything beyond this risks merging genuinely distinct
/// documents; near-duplicate detection handles softer similarity.
pub fn normalize_for_hash(text: &str) -> String {
    let nfc: String = text.nfc().collect();
    let mut out = String::with_capacity(nfc.len());
    let mut last_was_space = false;
    for ch in nfc.trim().chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
            last_was_space = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_whitespace_and_lowercases() {
        assert_eq!(normalize_for_hash("  Hello\t\nWORLD  "), "hello world");
    }

    #[test]
    fn stable_across_repeated_calls() {
        let a = normalize_for_hash("Soomaaliya  waa\tDAL");
        let b = normalize_for_hash("Soomaaliya waa dal");
        assert_eq!(a, b);
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(normalize_for_hash("   \n\t "), "");
    }
}
