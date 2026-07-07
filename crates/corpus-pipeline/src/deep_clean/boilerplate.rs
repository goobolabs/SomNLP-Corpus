//! Boilerplate line removal for deep clean.

use std::sync::OnceLock;

use common::types::QualityFlag;
use regex::Regex;

fn nav_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Z0-9|«»]{10,}$").unwrap())
}

fn pipe_menu_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\|.*\|$").unwrap())
}

fn phrase_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^(written by|posted by|tags:|click here|contact us|live help|live chat|share this)",
        )
        .unwrap()
    })
}

fn phone_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[\d\s\-+().]{7,}$").unwrap())
}

fn is_boilerplate_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if nav_re().is_match(trimmed) {
        return true;
    }
    if pipe_menu_re().is_match(trimmed) {
        return true;
    }
    if phrase_re().is_match(trimmed) {
        return true;
    }
    if phone_re().is_match(trimmed) {
        return true;
    }
    let words: Vec<_> = trimmed.split_whitespace().collect();
    words.len() < 4 && !contains_somali_function_word(trimmed)
}

fn contains_somali_function_word(line: &str) -> bool {
    const WORDS: &[&str] = &[
        "waa", "oo", "ka", "ku", "la", "ay", "uu", "iyo", "in", "aan", "waxa", "waxaa",
    ];
    let lower = line.to_lowercase();
    WORDS.iter().any(|w| lower.contains(w))
}

pub fn filter_boilerplate_lines(
    text: &str,
    enabled: bool,
    reject_ratio: f64,
) -> Result<String, QualityFlag> {
    if !enabled {
        return Ok(text.to_string());
    }
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Ok(text.to_string());
    }
    let line_count = lines.len();
    let mut kept = Vec::new();
    let mut dropped = 0usize;
    for line in lines {
        if is_boilerplate_line(line) {
            dropped += 1;
        } else {
            kept.push(line);
        }
    }
    let drop_ratio = dropped as f64 / line_count as f64;
    if drop_ratio > reject_ratio {
        return Err(QualityFlag::Boilerplate);
    }
    Ok(kept.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_nav_line() {
        let out = filter_boilerplate_lines(
            "HOMEGABAYODUCOOYIN\nSoomaaliya waa dal ku yaal Geeska Afrika oo leh taariikh dheer.",
            true,
            0.9,
        )
        .unwrap();
        assert!(out.contains("Soomaaliya"));
        assert!(!out.contains("HOMEGABAY"));
    }
}
