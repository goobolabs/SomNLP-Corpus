//! Clean-stage orchestration: run the cleaning chain on a [`RawRecord`] and
//! build a [`CorpusRecord`] with canonical hash/ID, provenance, license, and
//! quality disposition. See docs/CLEANING_PLAN.md (Clean stage).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use common::hash::{content_hash, make_doc_id};
use common::registry::{self, SourceClass};
use common::reject;
use common::types::{
    CorpusRecord, DedupInfo, Lang, Provenance, QualityFlag, QualityInfo, RawRecord, SCHEMA_VERSION,
};
use unicode_normalization::UnicodeNormalization;

use super::{entities, gates, heuristics, mojibake, repeated, strip, whitespace};
use crate::config::CleanConfig;

/// Language tag assigned at the clean stage; the LID stage replaces it.
const UNDETERMINED_LANG: &str = "und";

/// Run the full cleaning chain in the documented order.
pub fn clean_text(raw: &str, max_run: usize) -> String {
    let decoded = entities::decode_entities(raw);
    let demojibaked = mojibake::fix_mojibake(&decoded);
    let nfc: String = demojibaked.nfc().collect();
    let stripped = strip::strip_invisibles(&nfc);
    let collapsed = repeated::collapse_repeats(&stripped, max_run);
    whitespace::normalize_whitespace(&collapsed)
}

/// Outcome of cleaning a single record.
pub enum CleanResult {
    /// Source key missing or not in the registry; the record was skipped.
    Skipped,
    /// A processed record. Inspect `quality.disposition` for keep/reject/review.
    Processed(Box<CorpusRecord>),
}

/// Clean one raw record into a corpus record with full metadata.
pub fn clean_record(
    raw: &RawRecord,
    cfg: &CleanConfig,
    collected_at: DateTime<Utc>,
) -> CleanResult {
    let Some(source_str) = raw.source.as_deref() else {
        return CleanResult::Skipped;
    };
    let Some(entry) = registry::lookup(source_str) else {
        return CleanResult::Skipped;
    };

    let cleaned = clean_text(&raw.text, cfg.max_repeated_run);
    let min_words = match entry.class {
        SourceClass::Document => cfg.document_min_words,
        SourceClass::Sentence => cfg.sentence_min_words,
    };

    let hash = content_hash(&cleaned);
    let id = make_doc_id(&entry.source_key(), &hash);

    let mut record = CorpusRecord {
        id,
        text: cleaned.clone(),
        provenance: Provenance {
            source: entry.source_key(),
            collected_at,
            lang: Lang(UNDETERMINED_LANG.to_string()),
            source_url: None,
            title: None,
            author: None,
            published_at: None,
            tags: Vec::new(),
            subsource: None,
        },
        license: entry.license(),
        content_hash: hash.clone(),
        dedup: DedupInfo {
            content_hash: Some(hash),
            ..Default::default()
        },
        quality: QualityInfo::default(),
        schema_version: SCHEMA_VERSION,
        meta: BTreeMap::new(),
    };

    if let Some(flag) = gates::rejection_reason(&cleaned, min_words, cfg.ufffd_reject_ratio) {
        reject::reject(&mut record, flag);
        return CleanResult::Processed(Box::new(record));
    }

    let ratio = heuristics::symbol_ratio(&cleaned);
    record.quality.symbol_ratio = Some(ratio as f32);
    if ratio > cfg.symbol_ratio_review {
        reject::flag_for_review(&mut record, QualityFlag::HighSymbolRatio);
    }
    if heuristics::has_html_remnant(&cleaned) {
        reject::flag_for_review(&mut record, QualityFlag::HtmlRemnant);
    }
    if heuristics::mostly_numbers(&cleaned) {
        reject::flag_for_review(&mut record, QualityFlag::MostlyNumbers);
    }

    CleanResult::Processed(Box::new(record))
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::types::RecordDisposition;

    fn cfg() -> CleanConfig {
        CleanConfig {
            document_min_words: 25,
            sentence_min_words: 5,
            ufffd_reject_ratio: 0.005,
            max_repeated_run: 3,
            symbol_ratio_review: 0.5,
        }
    }

    #[test]
    fn full_chain_cleans_text() {
        let out = clean_text("itâ€™s   a  &amp; test\u{200B}!!!!!!", 3);
        assert_eq!(out, "it\u{2019}s a & test!!!");
    }

    #[test]
    fn short_document_is_rejected() {
        let raw = RawRecord {
            text: "Soomaaliya waa dal.".into(),
            source: Some("hplt".into()),
        };
        let CleanResult::Processed(record) = clean_record(&raw, &cfg(), Utc::now()) else {
            panic!("expected processed");
        };
        assert_eq!(record.quality.disposition, RecordDisposition::Rejected);
        assert!(record.quality.flags.contains(&QualityFlag::TooShort));
    }

    #[test]
    fn short_sentence_source_is_kept() {
        let raw = RawRecord {
            text: "Soomaaliya waa dal wanaagsan oo qurux badan.".into(),
            source: Some("opus".into()),
        };
        let CleanResult::Processed(record) = clean_record(&raw, &cfg(), Utc::now()) else {
            panic!("expected processed");
        };
        assert_eq!(record.quality.disposition, RecordDisposition::Kept);
        assert!(record.content_hash.0.len() == 64);
        assert!(record.id.0.starts_with("opus:"));
    }

    #[test]
    fn unknown_source_is_skipped() {
        let raw = RawRecord {
            text: "text".into(),
            source: Some("oscar".into()),
        };
        assert!(matches!(
            clean_record(&raw, &cfg(), Utc::now()),
            CleanResult::Skipped
        ));
    }
}
