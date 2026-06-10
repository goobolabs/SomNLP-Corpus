//! Word-k-gram shingling into a sorted, deduplicated array of 31-bit hashes.
//! See docs/CLEANING_PLAN.md (Near-deduplication).

use blake2::{Blake2b512, Digest};

/// Hash one shingle string to a 31-bit integer (matches the reference layout).
fn shingle_hash(shingle: &str) -> u64 {
    let digest = Blake2b512::digest(shingle.as_bytes());
    let v = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    (v & 0x7FFF_FFFF) as u64
}

/// Compute the set of word-`k`-gram shingle hashes for `text`, as a sorted,
/// deduplicated `Vec<u64>`. Normalization mirrors hashing: lowercase + whitespace
/// split. Returns empty when there are fewer than `k` tokens.
pub fn shingle_ints(text: &str, k: usize) -> Vec<u64> {
    let lower = text.to_lowercase();
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    if tokens.len() < k {
        return Vec::new();
    }
    let mut ids: Vec<u64> = tokens
        .windows(k)
        .map(|window| shingle_hash(&window.join(" ")))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Jaccard similarity of two sorted, deduplicated shingle arrays.
pub fn jaccard(a: &[u64], b: &[u64]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let (mut i, mut j, mut intersection) = (0usize, 0usize, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                intersection += 1;
                i += 1;
                j += 1;
            }
        }
    }
    let union = a.len() + b.len() - intersection;
    if union == 0 {
        1.0
    } else {
        intersection as f64 / union as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn too_few_tokens_yields_empty() {
        assert!(shingle_ints("one two", 3).is_empty());
    }

    #[test]
    fn identical_text_has_jaccard_one() {
        let text = "the quick brown fox jumps over the lazy dog every morning";
        let a = shingle_ints(text, 3);
        let b = shingle_ints(text, 3);
        assert_eq!(jaccard(&a, &b), 1.0);
    }

    #[test]
    fn disjoint_text_has_low_jaccard() {
        let a = shingle_ints("alpha beta gamma delta epsilon zeta", 3);
        let b = shingle_ints("one two three four five six seven", 3);
        assert!(jaccard(&a, &b) < 0.1);
    }

    #[test]
    fn near_identical_text_has_high_jaccard() {
        let a = shingle_ints("the quick brown fox jumps over the lazy dog today", 3);
        let b = shingle_ints("the quick brown fox jumps over the lazy dog now", 3);
        assert!(jaccard(&a, &b) > 0.6, "got {}", jaccard(&a, &b));
    }
}
