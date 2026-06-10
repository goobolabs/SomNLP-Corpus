//! Zero-cost quality heuristics that need no seed corpus. These flag records for
//! review (not rejection); thresholds are calibrated later. See
//! docs/CLEANING_PLAN.md §5.

use std::sync::OnceLock;

use regex::Regex;

fn html_tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"</?[a-zA-Z][a-zA-Z0-9]*(\s[^>]*)?>").unwrap())
}

/// Ratio of non-alphanumeric, non-whitespace characters to all non-whitespace
/// characters. High values indicate symbol spam or boilerplate.
pub fn symbol_ratio(text: &str) -> f64 {
    let non_ws = text.chars().filter(|c| !c.is_whitespace()).count();
    if non_ws == 0 {
        return 0.0;
    }
    let symbols = text
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_alphanumeric())
        .count();
    symbols as f64 / non_ws as f64
}

/// True when digits outnumber letters (listing pages, tables of numbers).
pub fn mostly_numbers(text: &str) -> bool {
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    let digits = text.chars().filter(|c| c.is_numeric()).count();
    digits > letters
}

/// True when the text still contains literal HTML tags.
pub fn has_html_remnant(text: &str) -> bool {
    html_tag_re().is_match(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_ratio_basic() {
        assert!(symbol_ratio("hello") < 0.01);
        assert!(symbol_ratio("!@#$%^") > 0.99);
    }

    #[test]
    fn detects_html_remnant() {
        assert!(has_html_remnant("text <p>para</p>"));
        assert!(has_html_remnant("line<br>break"));
        assert!(!has_html_remnant("2 < 3 and 5 > 4"));
    }

    #[test]
    fn mostly_numbers_detection() {
        assert!(mostly_numbers("12 34 56 78 9 a"));
        assert!(!mostly_numbers("waa sannad 2024"));
    }
}
