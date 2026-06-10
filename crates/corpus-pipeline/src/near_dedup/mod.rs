//! Near-deduplication: MinHash + LSH candidate generation, mandatory
//! exact-Jaccard verification, union-find clustering, and keep-longest. Applied
//! to document-class records only. See docs/CLEANING_PLAN.md (Near-deduplication).

pub mod lsh;
pub mod minhash;
pub mod shingle;
pub mod union_find;

use rustc_hash::FxHashMap;

use crate::config::NearDedupConfig;
use minhash::MinHasher;
use union_find::UnionFind;

#[derive(Debug, Default)]
pub struct DedupStats {
    pub candidate_pairs: usize,
    pub verified_pairs: usize,
    pub clusters: usize,
    pub removed: usize,
}

pub struct DedupOutcome {
    /// Removed document index -> kept canonical document index.
    pub removed_to_canonical: FxHashMap<usize, usize>,
    pub stats: DedupStats,
}

/// Run near-dedup over document-class shingle sets.
///
/// - `shingle_sets[i]` is the sorted, deduplicated shingle array for document `i`.
/// - `lengths[i]` is the length used for keep-longest (longer wins; ties go to the
///   smaller index).
pub fn near_dedup(
    shingle_sets: &[Vec<u64>],
    lengths: &[usize],
    cfg: &NearDedupConfig,
) -> DedupOutcome {
    let hasher = MinHasher::new(cfg.k_hashes, cfg.seed);
    let signatures: Vec<Vec<u64>> = shingle_sets.iter().map(|s| hasher.signature(s)).collect();

    let candidates = lsh::candidate_pairs(&signatures, cfg.bands, cfg.rows);
    let candidate_pairs = candidates.len();

    // Mandatory exact-Jaccard verification at the real threshold: LSH only yields
    // ~0.5-similarity candidates, so unverified removal would be too aggressive.
    let mut uf = UnionFind::new(shingle_sets.len());
    let mut verified_pairs = 0usize;
    for (i, j) in candidates {
        if shingle::jaccard(&shingle_sets[i], &shingle_sets[j]) >= cfg.tau {
            uf.union(i, j);
            verified_pairs += 1;
        }
    }

    let mut groups: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
    for idx in 0..shingle_sets.len() {
        let root = uf.find(idx);
        groups.entry(root).or_default().push(idx);
    }

    let mut removed_to_canonical = FxHashMap::default();
    let mut clusters = 0usize;
    for members in groups.values() {
        if members.len() < 2 {
            continue;
        }
        clusters += 1;
        let canonical = *members
            .iter()
            .max_by(|&&a, &&b| lengths[a].cmp(&lengths[b]).then(b.cmp(&a)))
            .expect("non-empty cluster");
        for &member in members {
            if member != canonical {
                removed_to_canonical.insert(member, canonical);
            }
        }
    }

    let removed = removed_to_canonical.len();
    DedupOutcome {
        removed_to_canonical,
        stats: DedupStats {
            candidate_pairs,
            verified_pairs,
            clusters,
            removed,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> NearDedupConfig {
        NearDedupConfig {
            shingle_k: 3,
            k_hashes: 64,
            bands: 16,
            rows: 4,
            tau: 0.80,
            seed: 0,
        }
    }

    #[test]
    fn removes_near_duplicate_keeps_longest() {
        let base = "the quick brown fox jumps over the lazy dog in the green field every";
        let near = format!("{base} morning");
        let other = "completely different sentence about something unrelated entirely here now today";

        let sets = vec![
            shingle::shingle_ints(base, 3),
            shingle::shingle_ints(&near, 3),
            shingle::shingle_ints(other, 3),
        ];
        let lengths = vec![base.len(), near.len(), other.len()];

        let outcome = near_dedup(&sets, &lengths, &cfg());
        assert_eq!(outcome.stats.removed, 1, "expected one near-duplicate removed");
        // The longer of the two near-duplicates (near) is canonical; base removed.
        assert!(outcome.removed_to_canonical.contains_key(&0));
        assert_eq!(outcome.removed_to_canonical[&0], 1);
        assert!(!outcome.removed_to_canonical.contains_key(&2));
    }

    #[test]
    fn distinct_documents_are_kept() {
        let sets = vec![
            shingle::shingle_ints("alpha beta gamma delta epsilon zeta eta theta", 3),
            shingle::shingle_ints("one two three four five six seven eight nine", 3),
        ];
        let lengths = vec![10, 10];
        let outcome = near_dedup(&sets, &lengths, &cfg());
        assert_eq!(outcome.stats.removed, 0);
    }
}
