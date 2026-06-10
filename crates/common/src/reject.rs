//! Helpers for marking records as rejected or held for review without deleting
//! them. Rejected records are written to sidecar files (see docs/QUALITY_METADATA.md).

use crate::types::{CorpusRecord, DocId, QualityFlag, RecordDisposition};

/// Mark a record as rejected, recording the quality flag that triggered it.
pub fn reject(record: &mut CorpusRecord, flag: QualityFlag) {
    record.quality.disposition = RecordDisposition::Rejected;
    if !record.quality.flags.contains(&flag) {
        record.quality.flags.push(flag);
    }
}

/// Mark a record as an exact duplicate of a canonical record and reject it.
pub fn reject_exact_duplicate(record: &mut CorpusRecord, canonical: DocId) {
    record.dedup.is_duplicate = true;
    record.dedup.duplicate_of = Some(canonical);
    record.quality.disposition = RecordDisposition::Rejected;
}

/// Mark a record as a near-duplicate of a cluster representative and reject it.
pub fn reject_near_duplicate(record: &mut CorpusRecord, canonical: DocId) {
    record.dedup.is_duplicate = true;
    record.dedup.near_duplicate_of = Some(canonical);
    record.quality.disposition = RecordDisposition::Rejected;
}

/// Add a quality flag and hold the record for manual review (not rejected).
pub fn flag_for_review(record: &mut CorpusRecord, flag: QualityFlag) {
    if !record.quality.flags.contains(&flag) {
        record.quality.flags.push(flag);
    }
    if record.quality.disposition == RecordDisposition::Kept {
        record.quality.disposition = RecordDisposition::Review;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ContentHash, Lang, License, Provenance, SourceKey, SCHEMA_VERSION};
    use chrono::Utc;
    use std::collections::BTreeMap;

    fn sample() -> CorpusRecord {
        CorpusRecord {
            id: DocId("hplt:0123456789abcdef".into()),
            text: "x".into(),
            provenance: Provenance {
                source: SourceKey("hplt".into()),
                collected_at: Utc::now(),
                lang: Lang("so".into()),
                source_url: None,
                title: None,
                author: None,
                published_at: None,
                tags: Vec::new(),
                subsource: None,
            },
            license: License::Cc0_1_0,
            content_hash: ContentHash("abc".into()),
            dedup: Default::default(),
            quality: Default::default(),
            schema_version: SCHEMA_VERSION,
            meta: BTreeMap::new(),
        }
    }

    #[test]
    fn reject_sets_disposition_and_flag() {
        let mut record = sample();
        reject(&mut record, QualityFlag::TooShort);
        assert_eq!(record.quality.disposition, RecordDisposition::Rejected);
        assert_eq!(record.quality.flags, vec![QualityFlag::TooShort]);
    }

    #[test]
    fn review_does_not_override_rejected() {
        let mut record = sample();
        reject(&mut record, QualityFlag::NotSomali);
        flag_for_review(&mut record, QualityFlag::HighSymbolRatio);
        assert_eq!(record.quality.disposition, RecordDisposition::Rejected);
    }

    #[test]
    fn near_duplicate_records_canonical() {
        let mut record = sample();
        reject_near_duplicate(&mut record, DocId("hplt:ffff".into()));
        assert!(record.dedup.is_duplicate);
        assert_eq!(record.dedup.near_duplicate_of, Some(DocId("hplt:ffff".into())));
    }
}
