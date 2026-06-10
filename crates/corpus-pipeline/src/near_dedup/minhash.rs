//! MinHash signatures via universal hashing `(a*x + b) mod prime`, with a fixed
//! seed for reproducibility. See docs/CLEANING_PLAN.md (Near-deduplication).

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Mersenne prime 2^31 - 1; shingle ids are 31-bit so products stay below 2^62.
const P_PRIME: u64 = (1 << 31) - 1;

pub struct MinHasher {
    a: Vec<u64>,
    b: Vec<u64>,
}

impl MinHasher {
    pub fn new(k: usize, seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let a = (0..k).map(|_| rng.gen_range(1..P_PRIME)).collect();
        let b = (0..k).map(|_| rng.gen_range(0..P_PRIME)).collect();
        Self { a, b }
    }

    pub fn k(&self) -> usize {
        self.a.len()
    }

    /// Compute the MinHash signature for a set of shingle ids. Empty input yields
    /// an all-`u64::MAX` signature (matches no other document).
    pub fn signature(&self, ids: &[u64]) -> Vec<u64> {
        let k = self.a.len();
        let mut sig = vec![u64::MAX; k];
        if ids.is_empty() {
            return sig;
        }
        for &id in ids {
            for i in 0..k {
                let h = (self.a[i].wrapping_mul(id).wrapping_add(self.b[i])) % P_PRIME;
                if h < sig[i] {
                    sig[i] = h;
                }
            }
        }
        sig
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_with_same_seed() {
        let h1 = MinHasher::new(64, 0);
        let h2 = MinHasher::new(64, 0);
        let ids = [1u64, 5, 9, 42, 1000];
        assert_eq!(h1.signature(&ids), h2.signature(&ids));
    }

    #[test]
    fn identical_sets_share_signature() {
        let h = MinHasher::new(64, 7);
        let ids = [3u64, 8, 15, 16, 23, 42];
        assert_eq!(h.signature(&ids), h.signature(&ids));
    }

    #[test]
    fn signature_estimates_jaccard() {
        let h = MinHasher::new(128, 1);
        let a: Vec<u64> = (0..100).collect();
        let b: Vec<u64> = (0..100).map(|x| x + 50).collect();
        let sig_a = h.signature(&a);
        let sig_b = h.signature(&b);
        let matches = sig_a.iter().zip(&sig_b).filter(|(x, y)| x == y).count();
        let estimate = matches as f64 / 128.0;
        // True Jaccard of [0,100) and [50,150) is 50/150 = 0.333.
        assert!((estimate - 0.333).abs() < 0.15, "estimate {estimate}");
    }
}
