//! LSH banding: group documents whose signatures collide in any band, producing
//! candidate pairs for exact-Jaccard verification. See docs/CLEANING_PLAN.md.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHashSet};

fn band_key(band: usize, slice: &[u64]) -> u64 {
    let mut hasher = DefaultHasher::new();
    band.hash(&mut hasher);
    slice.hash(&mut hasher);
    hasher.finish()
}

/// Generate candidate pairs `(i, j)` with `i < j` whose signatures share at least
/// one band bucket. `bands * rows` must equal the signature length.
pub fn candidate_pairs(signatures: &[Vec<u64>], bands: usize, rows: usize) -> Vec<(usize, usize)> {
    let mut pairs: FxHashSet<(usize, usize)> = FxHashSet::default();

    for band in 0..bands {
        let start = band * rows;
        let mut buckets: FxHashMap<u64, Vec<usize>> = FxHashMap::default();
        for (doc, sig) in signatures.iter().enumerate() {
            let slice = &sig[start..start + rows];
            buckets.entry(band_key(band, slice)).or_default().push(doc);
        }
        for docs in buckets.values() {
            if docs.len() < 2 {
                continue;
            }
            for i in 0..docs.len() {
                for j in (i + 1)..docs.len() {
                    let (a, b) = (docs[i], docs[j]);
                    pairs.insert(if a < b { (a, b) } else { (b, a) });
                }
            }
        }
    }

    pairs.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_signatures_are_candidates() {
        let sig = vec![1u64, 2, 3, 4, 5, 6, 7, 8];
        let signatures = vec![sig.clone(), sig.clone(), vec![9, 9, 9, 9, 9, 9, 9, 9]];
        let pairs = candidate_pairs(&signatures, 4, 2);
        assert!(pairs.contains(&(0, 1)));
        assert!(!pairs.contains(&(0, 2)));
    }
}
