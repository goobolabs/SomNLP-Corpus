//! Collapse pathological character repetitions while preserving natural Somali
//! spelling. Letters and punctuation collapse to `max_run`; digits are never
//! touched (collapsing them corrupts numbers). See docs/CLEANING_PLAN.md.

fn collapsible(c: char) -> bool {
    c.is_alphabetic() || c.is_ascii_punctuation()
}

/// Collapse runs of 4+ identical letters or punctuation down to `max_run`
/// characters. Digits and whitespace runs are left untouched.
pub fn collapse_repeats(text: &str, max_run: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev: Option<char> = None;
    let mut run = 0usize;
    for c in text.chars() {
        if Some(c) == prev {
            run += 1;
        } else {
            run = 1;
            prev = Some(c);
        }
        if collapsible(c) && run > max_run {
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_natural_doublets_and_triples() {
        assert_eq!(collapse_repeats("waaa", 3), "waaa");
        assert_eq!(collapse_repeats("waa", 3), "waa");
    }

    #[test]
    fn collapses_long_letter_runs() {
        assert_eq!(collapse_repeats("waaaaaaa", 3), "waaa");
    }

    #[test]
    fn collapses_punctuation_runs() {
        assert_eq!(collapse_repeats("stop!!!!!!!", 3), "stop!!!");
    }

    #[test]
    fn never_collapses_digits() {
        assert_eq!(collapse_repeats("10000000", 3), "10000000");
    }
}
