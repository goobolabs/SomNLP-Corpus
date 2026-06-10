//! Content hashing for exact dedup and document IDs (see docs/ID_STRATEGY.md).

use sha2::{Digest, Sha256};

use crate::normalize::normalize_for_hash;
use crate::types::{ContentHash, DocId, SourceKey};

/// Number of leading hex characters of the content hash used in a [`DocId`].
pub const DOC_ID_HASH_PREFIX_LEN: usize = 16;

/// SHA-256 hex digest of the normalized text. This is the canonical
/// `content_hash` for a record.
pub fn content_hash(text: &str) -> ContentHash {
    let normalized = normalize_for_hash(text);
    let digest = Sha256::digest(normalized.as_bytes());
    ContentHash(hex_lower(&digest))
}

/// Build a stable document ID as `{source_key}:{hash_prefix}`.
pub fn make_doc_id(source: &SourceKey, hash: &ContentHash) -> DocId {
    let prefix: String = hash.0.chars().take(DOC_ID_HASH_PREFIX_LEN).collect();
    DocId(format!("{}:{}", source.0, prefix))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(nibble(byte >> 4));
        out.push(nibble(byte & 0x0f));
    }
    out
}

fn nibble(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        _ => (b'a' + value - 10) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_64_hex_chars() {
        let h = content_hash("Soomaaliya");
        assert_eq!(h.0.len(), 64);
        assert!(h.0.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_ignores_normalization_differences() {
        assert_eq!(content_hash("Hello  World"), content_hash("hello world"));
    }

    #[test]
    fn doc_id_combines_source_and_prefix() {
        let hash = content_hash("text");
        let id = make_doc_id(&SourceKey("hplt".into()), &hash);
        let expected_prefix: String = hash.0.chars().take(DOC_ID_HASH_PREFIX_LEN).collect();
        assert_eq!(id.0, format!("hplt:{expected_prefix}"));
    }
}
