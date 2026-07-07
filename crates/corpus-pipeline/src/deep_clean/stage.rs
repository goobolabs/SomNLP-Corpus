//! Deep-clean stage orchestration. See docs/CLEANING_STRATEGY.md §5.

use common::hash::content_hash;
use common::registry::{self, SourceClass};
use common::types::{CorpusRecord, QualityFlag, RecordDisposition};
use unicode_normalization::UnicodeNormalization;

use super::boilerplate;
use super::contact;
use super::html;
use super::intra_dedup;
use super::lid;
use super::normalize;
use super::DeepCleanConfig;
use crate::clean::{entities, heuristics, mojibake, repeated, strip, whitespace};
use crate::config::CleanConfig;
use crate::lid::Detector;

pub struct DeepCleanResult {
    pub record: CorpusRecord,
    pub reject: Option<QualityFlag>,
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn apply_heuristics_v2(
    record: &mut CorpusRecord,
    text: &str,
    cfg: &DeepCleanConfig,
    clean_cfg: &CleanConfig,
) -> Option<QualityFlag> {
    let source = record.provenance.source.0.as_str();
    let class = registry::lookup(source)
        .map(|e| e.class)
        .unwrap_or(SourceClass::Document);

    if class == SourceClass::Document && word_count(text) > cfg.max_document_words {
        return Some(QualityFlag::TooLong);
    }

    let ratio = heuristics::symbol_ratio(text);
    record.quality.symbol_ratio = Some(ratio as f32);
    if ratio > cfg.symbol_ratio_reject {
        return Some(QualityFlag::HighSymbolRatio);
    }
    if ratio > cfg.symbol_ratio_review {
        record.quality.disposition = RecordDisposition::Review;
        if !record.quality.flags.contains(&QualityFlag::HighSymbolRatio) {
            record.quality.flags.push(QualityFlag::HighSymbolRatio);
        }
    }

    if heuristics::mostly_numbers(text) {
        if class == SourceClass::Document {
            return Some(QualityFlag::MostlyNumbers);
        }
    }

    if heuristics::has_html_remnant(text) {
        if cfg.reject_script_html || !cfg.strip_benign_html {
            return Some(QualityFlag::HtmlRemnant);
        }
        if record.quality.disposition == RecordDisposition::Kept {
            record.quality.disposition = RecordDisposition::Review;
        }
        if !record.quality.flags.contains(&QualityFlag::HtmlRemnant) {
            record.quality.flags.push(QualityFlag::HtmlRemnant);
        }
    }

    let _ = clean_cfg;
    None
}

pub fn deep_clean_record(
    mut record: CorpusRecord,
    cfg: &DeepCleanConfig,
    clean_cfg: &CleanConfig,
    detector: &dyn Detector,
) -> DeepCleanResult {
    if record.quality.disposition == RecordDisposition::Rejected {
        return DeepCleanResult {
            record,
            reject: None,
        };
    }

    let source = record.provenance.source.0.clone();
    let mut text = record.text.clone();

    text = normalize::normalize_source_text(
        &source,
        &text,
        cfg.unescape_madlad,
        cfg.strip_opus_html,
    );
    text = entities::decode_entities(&text);
    text = mojibake::fix_mojibake_with_passes(&text, cfg.mojibake_max_passes);
    let nfc: String = text.nfc().collect();
    text = strip::strip_invisibles(&nfc);
    if cfg.strip_stray_ufffd {
        text = strip::strip_stray_ufffd(&text);
    }
    text = repeated::collapse_repeats(&text, clean_cfg.max_repeated_run);
    text = whitespace::normalize_whitespace(&text);

    text = match html::apply_html_policy(&text, cfg.reject_script_html, cfg.strip_benign_html) {
        Ok(stripped) => stripped,
        Err(flag) => {
            return DeepCleanResult {
                record,
                reject: Some(flag),
            };
        }
    };
    text = whitespace::normalize_whitespace(&text);

    text = match boilerplate::filter_boilerplate_lines(
        &text,
        cfg.boilerplate_line_drop,
        cfg.boilerplate_reject_ratio,
    ) {
        Ok(filtered) => filtered,
        Err(flag) => {
            return DeepCleanResult {
                record,
                reject: Some(flag),
            };
        }
    };

    text = contact::mask_contacts(&text, cfg.mask_urls, cfg.mask_emails);
    text = intra_dedup::dedup_paragraphs(
        &text,
        cfg.intra_dedup.enabled,
        cfg.intra_dedup.paragraph_jaccard_tau,
    );
    text = whitespace::normalize_whitespace(&text);

    if let Err(flag) = lid::check_segment_lid(&source, &text, &cfg.lid, detector) {
        return DeepCleanResult {
            record,
            reject: Some(flag),
        };
    }

    record.text = text.clone();
    let hash = content_hash(&text);
    record.content_hash = hash.clone();
    record.dedup.content_hash = Some(hash);

    if let Some(flag) = apply_heuristics_v2(&mut record, &text, cfg, clean_cfg) {
        return DeepCleanResult {
            record,
            reject: Some(flag),
        };
    }

    DeepCleanResult {
        record,
        reject: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use common::hash::content_hash;
    use common::types::{
        DedupInfo, DocId, Lang, License, Provenance, QualityInfo, SourceKey,
        SCHEMA_VERSION,
    };
    use std::collections::BTreeMap;

    use crate::lid::LinguaDetector;

    fn sample_record(source: &str, text: &str) -> CorpusRecord {
        let hash = content_hash(text);
        CorpusRecord {
            id: DocId(format!("{source}:abc")),
            text: text.to_string(),
            provenance: Provenance {
                source: SourceKey(source.into()),
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
            content_hash: hash.clone(),
            dedup: DedupInfo {
                content_hash: Some(hash),
                ..Default::default()
            },
            quality: QualityInfo::default(),
            schema_version: SCHEMA_VERSION,
            meta: BTreeMap::new(),
        }
    }

    fn configs() -> (DeepCleanConfig, CleanConfig) {
        (
            DeepCleanConfig::default(),
            CleanConfig {
                document_min_words: 25,
                sentence_min_words: 5,
                ufffd_reject_ratio: 0.005,
                max_repeated_run: 3,
                symbol_ratio_review: 0.5,
            },
        )
    }

    #[test]
    fn unescapes_madlad_in_deep_clean() {
        let (mut dc, clean) = configs();
        dc.lid.segment_level = false;
        let detector = LinguaDetector::new();
        let text = "Soomaaliya waa dal wanaagsan oo leh taariikh dheer.\nline1\\nline2";
        let result = deep_clean_record(
            sample_record("madlad", text),
            &dc,
            &clean,
            &detector,
        );
        assert!(result.reject.is_none(), "{:?}", result.record.quality.flags);
        assert!(result.record.text.contains('\n'));
    }
}
