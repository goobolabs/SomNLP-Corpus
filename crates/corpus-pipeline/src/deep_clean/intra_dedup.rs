//! Intra-document paragraph deduplication.

use std::collections::HashSet;

fn word_set(text: &str) -> HashSet<String> {
    text.split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() > 2)
        .collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let inter = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

pub fn dedup_paragraphs(text: &str, enabled: bool, tau: f64) -> String {
    if !enabled {
        return text.to_string();
    }
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    if paragraphs.len() <= 1 {
        return text.to_string();
    }
    let mut kept = Vec::new();
    let mut prev_set: Option<HashSet<String>> = None;
    for para in paragraphs {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            continue;
        }
        let set = word_set(trimmed);
        let duplicate = prev_set
            .as_ref()
            .is_some_and(|prev| jaccard(prev, &set) >= tau);
        if !duplicate {
            kept.push(trimmed);
            prev_set = Some(set);
        }
    }
    kept.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_duplicate_paragraph() {
        let text = "Wax wanaagsan oo dheer\n\nWax wanaagsan oo dheer\n\nKale";
        let out = dedup_paragraphs(text, true, 0.95);
        assert_eq!(out.matches("Wax wanaagsan").count(), 1);
    }
}
