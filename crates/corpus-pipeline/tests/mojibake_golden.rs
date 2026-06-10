//! Golden test: every fixture pair must be repaired exactly, and clean text must
//! pass through untouched. See docs/CLEANING_PLAN.md §2.

use std::path::PathBuf;

use corpus_pipeline::clean::mojibake::fix_mojibake;
use serde::Deserialize;

#[derive(Deserialize)]
struct Pair {
    corrupted: String,
    expected: String,
}

#[test]
fn repairs_all_golden_pairs() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mojibake.jsonl");
    let text = std::fs::read_to_string(&path).expect("read mojibake fixture");

    let mut count = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let pair: Pair = serde_json::from_str(line).expect("parse fixture line");
        let fixed = fix_mojibake(&pair.corrupted);
        assert_eq!(
            fixed, pair.expected,
            "mojibake repair mismatch for {:?}",
            pair.corrupted
        );
        count += 1;
    }
    assert!(count >= 20, "expected a substantial fixture, got {count}");
}

#[test]
fn does_not_corrupt_clean_text() {
    for clean in [
        "Soomaaliya waa dal ku yaal Geeska Afrika.",
        "café crème",
        "naïve résumé",
        "plain ascii text 12345",
    ] {
        assert_eq!(fix_mojibake(clean), clean, "altered clean text: {clean:?}");
    }
}
