//! Stage 4 — normalize and validate extracted records.
//!
//! Cleaning steps
//! * Unicode normalization (NFC) so composed/decomposed forms compare equal.
//! * Removal of control / zero-width / bidi-format characters that break
//!   downstream tooling.
//! * Whitespace collapse (every run of whitespace becomes one space) and trim,
//!   which also guarantees one document per line in the plain-text export.
//! * Rejection of empty, too-short, or mojibake-corrupted records.
//!
//! Somali is written in the Latin script, so normalization deliberately keeps
//! every letter, apostrophe (glottal stop) and diacritic intact — only
//! genuinely non-textual code points are removed.

use unicode_normalization::UnicodeNormalization;

use crate::xlsum::record::XlsumRecord;

/// U+FFFD; a high proportion of these signals a decoding failure upstream.
const REPLACEMENT_CHAR: char = '\u{FFFD}';

/// Share of U+FFFD above which a value is treated as corrupted.
pub const MAX_REPLACEMENT_RATIO: f64 = 0.02;

/// Gates a cleaned record must clear to be kept.
#[derive(Debug, Clone, Copy)]
pub struct CleanOptions {
    pub min_text_chars: usize,
    pub min_title_chars: usize,
    pub require_summary: bool,
    pub max_replacement_ratio: f64,
}

impl Default for CleanOptions {
    fn default() -> Self {
        Self {
            min_text_chars: 20,
            min_title_chars: 1,
            require_summary: true,
            max_replacement_ratio: MAX_REPLACEMENT_RATIO,
        }
    }
}

/// Why a record was dropped, for the stage summary.
#[derive(Debug, Default, Clone, Copy)]
pub struct DropCounts {
    pub too_short: u64,
    pub missing_title: u64,
    pub missing_summary: u64,
    pub corrupted: u64,
}

impl DropCounts {
    pub fn total(&self) -> u64 {
        self.too_short + self.missing_title + self.missing_summary + self.corrupted
    }
}

/// NFC-normalize, strip non-textual code points, collapse whitespace, trim.
pub fn clean_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let normalized: String = text.nfc().filter(|ch| !is_stripped(*ch)).collect();

    let mut out = String::with_capacity(normalized.len());
    let mut last_was_space = false;
    for ch in normalized.trim().chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(ch);
            last_was_space = false;
        }
    }
    out
}

/// Control, zero-width, BOM and bidi-format code points.
///
/// Real newlines and tabs survive this pass and are collapsed to spaces by the
/// whitespace pass, matching the reference Python cleaner.
fn is_stripped(ch: char) -> bool {
    if ch == '\n' || ch == '\t' {
        return false;
    }
    if ch.is_control() {
        return true;
    }
    matches!(
        ch,
        '\u{00AD}'                 // soft hyphen
            | '\u{200B}'..='\u{200F}'  // zero-width space/joiners, LRM/RLM
            | '\u{202A}'..='\u{202E}'  // bidi embedding/override
            | '\u{2060}'..='\u{2064}'  // word joiner, invisible operators
            | '\u{FEFF}'               // BOM / zero-width no-break space
            | '\u{FFF9}'..='\u{FFFB}' // interlinear annotation
    )
}

/// Whether `text` looks mojibake-corrupted.
pub fn is_corrupted(text: &str, max_replacement_ratio: f64) -> bool {
    if text.is_empty() {
        return false;
    }
    let total = text.chars().count() as f64;
    let replacements = text.chars().filter(|ch| *ch == REPLACEMENT_CHAR).count() as f64;
    replacements / total > max_replacement_ratio
}

/// Clean one record, returning `None` when it fails a gate.
pub fn clean_record(
    record: &XlsumRecord,
    options: &CleanOptions,
    drops: &mut DropCounts,
) -> Option<XlsumRecord> {
    let title = clean_text(&record.title);
    let text = clean_text(&record.text);
    let summary = clean_text(&record.summary);

    if text.chars().count() < options.min_text_chars {
        drops.too_short += 1;
        return None;
    }
    if title.chars().count() < options.min_title_chars {
        drops.missing_title += 1;
        return None;
    }
    if options.require_summary && summary.is_empty() {
        drops.missing_summary += 1;
        return None;
    }
    let ratio = options.max_replacement_ratio;
    if is_corrupted(&text, ratio) || is_corrupted(&title, ratio) || is_corrupted(&summary, ratio) {
        drops.corrupted += 1;
        return None;
    }

    Some(XlsumRecord {
        id: record.id.clone(),
        url: record.url.clone(),
        split: record.split.clone(),
        title,
        text,
        summary,
    })
}

/// Clean a batch of records, dropping the invalid ones.
pub fn clean_records(
    records: &[XlsumRecord],
    options: &CleanOptions,
) -> (Vec<XlsumRecord>, DropCounts) {
    let mut cleaned = Vec::with_capacity(records.len());
    let mut drops = DropCounts::default();

    for record in records {
        if let Some(record) = clean_record(record, options, &mut drops) {
            cleaned.push(record);
        }
    }

    println!(
        "Cleaned {} record(s); dropped {} (short: {}, no title: {}, no summary: {}, corrupted: {}).",
        cleaned.len(),
        drops.total(),
        drops.too_short,
        drops.missing_title,
        drops.missing_summary,
        drops.corrupted
    );
    (cleaned, drops)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(text: &str) -> XlsumRecord {
        XlsumRecord {
            id: "1".into(),
            url: "https://example.test/1".into(),
            split: "train".into(),
            title: "Cinwaan".into(),
            text: text.into(),
            summary: "Kooban".into(),
        }
    }

    #[test]
    fn collapses_whitespace_and_trims() {
        assert_eq!(
            clean_text("  Suudaan   oo\nbaadigoobaysa  "),
            "Suudaan oo baadigoobaysa"
        );
    }

    #[test]
    fn strips_zero_width_and_bom() {
        assert_eq!(clean_text("Soo\u{200B}maali\u{FEFF}ya"), "Soomaaliya");
    }

    #[test]
    fn preserves_somali_letters_and_glottal_stop() {
        let text = "Ma'aan arag shaqaale'da Soomaaliyeed — 'daawo'";
        assert_eq!(clean_text(text), text);
    }

    #[test]
    fn nfc_composes_decomposed_forms() {
        // "é" written as e + combining acute must equal the composed form.
        assert_eq!(clean_text("cafe\u{0301}"), clean_text("caf\u{00E9}"));
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(clean_text("   \n\t "), "");
    }

    #[test]
    fn corruption_needs_more_than_the_ratio() {
        let mostly_fine = format!("{}\u{FFFD}", "a".repeat(200));
        assert!(!is_corrupted(&mostly_fine, MAX_REPLACEMENT_RATIO));
        assert!(is_corrupted("\u{FFFD}\u{FFFD}ab", MAX_REPLACEMENT_RATIO));
        assert!(!is_corrupted("", MAX_REPLACEMENT_RATIO));
    }

    #[test]
    fn short_text_is_dropped() {
        let mut drops = DropCounts::default();
        assert!(clean_record(&record("gaaban"), &CleanOptions::default(), &mut drops).is_none());
        assert_eq!(drops.too_short, 1);
    }

    #[test]
    fn missing_summary_is_dropped_unless_optional() {
        let mut source = record("Qoraal dheer oo ku filan gudbinta xadka.");
        source.summary = String::new();

        let mut drops = DropCounts::default();
        assert!(clean_record(&source, &CleanOptions::default(), &mut drops).is_none());
        assert_eq!(drops.missing_summary, 1);

        let options = CleanOptions {
            require_summary: false,
            ..CleanOptions::default()
        };
        let mut drops = DropCounts::default();
        assert!(clean_record(&source, &options, &mut drops).is_some());
    }

    #[test]
    fn valid_record_keeps_its_provenance() {
        let mut drops = DropCounts::default();
        let cleaned = clean_record(
            &record("  Qoraal   dheer oo ku filan gudbinta xadka.  "),
            &CleanOptions::default(),
            &mut drops,
        )
        .unwrap();
        assert_eq!(cleaned.text, "Qoraal dheer oo ku filan gudbinta xadka.");
        assert_eq!(cleaned.url, "https://example.test/1");
        assert_eq!(cleaned.split, "train");
        assert_eq!(drops.total(), 0);
    }

    #[test]
    fn min_text_chars_counts_characters_not_bytes() {
        // 21 two-byte characters: over the 20-char gate, under it if counted wrong.
        let mut drops = DropCounts::default();
        let text = "é".repeat(21);
        assert!(clean_record(&record(&text), &CleanOptions::default(), &mut drops).is_some());
    }
}
