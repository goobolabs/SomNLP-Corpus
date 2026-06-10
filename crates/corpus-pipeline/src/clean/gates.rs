//! Rejection gates applied after the cleaning chain: empty, corruption, and
//! length floors. See docs/CLEANING_PLAN.md (Clean stage).

use common::types::QualityFlag;

/// Whitespace-separated word count.
pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Fraction of characters that are U+FFFD replacement characters.
pub fn ufffd_ratio(text: &str) -> f64 {
    let total = text.chars().count();
    if total == 0 {
        return 0.0;
    }
    let bad = text.chars().filter(|&c| c == '\u{FFFD}').count();
    bad as f64 / total as f64
}

/// Return the first rejection reason, if the record should be rejected.
pub fn rejection_reason(
    text: &str,
    min_words: usize,
    ufffd_reject_ratio: f64,
) -> Option<QualityFlag> {
    if text.trim().is_empty() {
        return Some(QualityFlag::TooShort);
    }
    if ufffd_ratio(text) > ufffd_reject_ratio {
        return Some(QualityFlag::Corrupted);
    }
    if word_count(text) < min_words {
        return Some(QualityFlag::TooShort);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert_eq!(rejection_reason("   ", 5, 0.005), Some(QualityFlag::TooShort));
    }

    #[test]
    fn rejects_corrupted() {
        let text = "a\u{FFFD}\u{FFFD}\u{FFFD}b";
        assert_eq!(rejection_reason(text, 1, 0.005), Some(QualityFlag::Corrupted));
    }

    #[test]
    fn rejects_below_floor() {
        assert_eq!(
            rejection_reason("one two three", 50, 0.005),
            Some(QualityFlag::TooShort)
        );
    }

    #[test]
    fn accepts_long_enough() {
        let text = "word ".repeat(60);
        assert_eq!(rejection_reason(&text, 50, 0.005), None);
    }
}
