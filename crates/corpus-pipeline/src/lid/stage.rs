//! LID-stage policy: gate document-class records on Somali confidence; tag-only
//! for sentence-class records. See docs/CLEANING_PLAN.md §3.

use common::registry::{self, LidPolicy};
use common::reject;
use common::types::{CorpusRecord, Lang, QualityFlag, RecordDisposition};

use super::Detector;

const SOMALI: &str = "so";

/// Apply language identification to a record in place. Sets `provenance.lang`
/// and `quality.lang_score`; rejects document-class non-Somali records.
pub fn apply_lid(
    record: &mut CorpusRecord,
    detector: &dyn Detector,
    min_confidence: f64,
    clip_bytes: usize,
) {
    let policy = registry::lookup(&record.provenance.source.0)
        .map(|entry| entry.lid_policy)
        .unwrap_or(LidPolicy::Gate);

    let snippet = clip(&record.text, clip_bytes);
    let detected = detector.detect(snippet);
    let (code, confidence) = detected
        .map(|(code, conf)| (Some(code), conf))
        .unwrap_or((None, 0.0));
    record.quality.lang_score = Some(confidence as f32);

    let is_somali = code.as_deref() == Some(SOMALI);

    match policy {
        LidPolicy::TagOnly => {
            // Curated parallel data: record the score, never reject.
            if let Some(code) = code {
                record.provenance.lang = Lang(code);
            }
        }
        LidPolicy::Gate => {
            if is_somali && confidence >= min_confidence {
                record.provenance.lang = Lang(SOMALI.to_string());
            } else if !is_somali {
                if let Some(code) = code {
                    record.provenance.lang = Lang(code);
                }
                reject::reject(record, QualityFlag::NotSomali);
            } else {
                // Somali but below the confidence threshold.
                record.provenance.lang = Lang(SOMALI.to_string());
                reject::reject(record, QualityFlag::LowLangScore);
            }
        }
    }
}

/// True if the record is still kept after LID.
pub fn is_kept(record: &CorpusRecord) -> bool {
    record.quality.disposition != RecordDisposition::Rejected
}

fn clip(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lid::LinguaDetector;
    use common::hash::content_hash;
    use common::types::{
        ContentHash, DocId, License, Provenance, QualityInfo, SourceKey, SCHEMA_VERSION,
    };
    use chrono::Utc;
    use std::collections::BTreeMap;

    fn record(source: &str, text: &str) -> CorpusRecord {
        let hash = content_hash(text);
        CorpusRecord {
            id: DocId(format!("{source}:x")),
            text: text.to_string(),
            provenance: Provenance {
                source: SourceKey(source.into()),
                collected_at: Utc::now(),
                lang: Lang("und".into()),
                source_url: None,
                title: None,
                author: None,
                published_at: None,
                tags: Vec::new(),
                subsource: None,
            },
            license: License::Cc0_1_0,
            content_hash: hash.clone(),
            dedup: Default::default(),
            quality: QualityInfo::default(),
            schema_version: SCHEMA_VERSION,
            meta: BTreeMap::new(),
        }
    }

    #[test]
    fn gate_keeps_somali_document() {
        let d = LinguaDetector::new();
        let mut r = record(
            "hplt",
            "Soomaaliya waa dal ku yaal Geeska Afrika oo leh dhaqan hodan ah iyo taariikh dheer.",
        );
        apply_lid(&mut r, &d, 0.5, 2000);
        assert!(is_kept(&r), "expected Somali doc kept");
        assert_eq!(r.provenance.lang.0, "so");
        assert!(r.quality.lang_score.is_some());
    }

    #[test]
    fn gate_rejects_english_document() {
        let d = LinguaDetector::new();
        let mut r = record(
            "hplt",
            "This is a clearly English sentence that should not be classified as Somali at all.",
        );
        apply_lid(&mut r, &d, 0.5, 2000);
        assert!(!is_kept(&r), "expected English doc rejected");
        assert!(r.quality.flags.contains(&QualityFlag::NotSomali));
    }

    #[test]
    fn tag_only_never_rejects_sentence_source() {
        let d = LinguaDetector::new();
        let mut r = record("opus", "This is English text in a parallel sentence corpus.");
        apply_lid(&mut r, &d, 0.5, 2000);
        assert!(is_kept(&r), "sentence source must not be rejected");
        assert!(r.quality.lang_score.is_some());
    }
}
