//! Stage 6 — corpus-level metadata report over the cleaned records.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::stats::{format_float, format_number};
use crate::xlsum::config::{DATASET_CONFIG, DATASET_NAME};
use crate::xlsum::record::XlsumRecord;

/// Estimated subword tokens per whitespace-delimited word. ~1.53 is a
/// reasonable estimate for Somali's Latin orthography under a modern
/// multilingual subword tokenizer.
pub const TOKENS_PER_WORD: f64 = 1.53;

/// Statistics over the cleaned `text` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusReport {
    pub repo: String,
    pub config: String,
    pub total_documents: u64,
    pub total_characters: u64,
    pub total_words: u64,
    pub estimated_tokens: u64,
    pub vocabulary_size: u64,
    pub average_article_length_chars: f64,
    pub average_article_length_words: f64,
    pub tokens_per_word: f64,
    pub tokens_estimation_basis: String,
}

/// Compute the report without writing it.
pub fn compute_stats(records: &[XlsumRecord], tokens_per_word: f64) -> Result<CorpusReport> {
    let word_re = Regex::new(r"\w+").context("compiling word regex")?;

    let total_docs = records.len() as u64;
    let mut total_chars = 0u64;
    let mut total_words = 0u64;
    let mut vocabulary: HashSet<String> = HashSet::new();

    for record in records {
        total_chars += record.text.chars().count() as u64;
        for word in word_re.find_iter(&record.text.to_lowercase()) {
            total_words += 1;
            vocabulary.insert(word.as_str().to_string());
        }
    }

    let (avg_chars, avg_words) = if total_docs == 0 {
        (0.0, 0.0)
    } else {
        (
            total_chars as f64 / total_docs as f64,
            total_words as f64 / total_docs as f64,
        )
    };

    Ok(CorpusReport {
        repo: DATASET_NAME.to_string(),
        config: DATASET_CONFIG.to_string(),
        total_documents: total_docs,
        total_characters: total_chars,
        total_words,
        estimated_tokens: (total_words as f64 * tokens_per_word).round() as u64,
        vocabulary_size: vocabulary.len() as u64,
        average_article_length_chars: round2(avg_chars),
        average_article_length_words: round2(avg_words),
        tokens_per_word,
        tokens_estimation_basis: format!("total_words * {tokens_per_word}"),
    })
}

/// Compute the report, print it, and write it as JSON.
pub fn generate_report(
    path: &Path,
    records: &[XlsumRecord],
    tokens_per_word: f64,
) -> Result<CorpusReport> {
    let report = compute_stats(records, tokens_per_word)?;
    print_report(&report);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&report)? + "\n")
        .with_context(|| format!("writing {}", path.display()))?;
    println!("Metadata report -> {}", path.display());

    Ok(report)
}

fn print_report(report: &CorpusReport) {
    println!();
    println!("{}", "=".repeat(48));
    println!("XL-Sum Somali corpus metadata");
    println!("{}", "=".repeat(48));
    println!(
        "Documents            : {}",
        format_number(report.total_documents)
    );
    println!(
        "Characters           : {}",
        format_number(report.total_characters)
    );
    println!(
        "Words                : {}",
        format_number(report.total_words)
    );
    println!(
        "Tokens (x{:.2})       : {}",
        report.tokens_per_word,
        format_number(report.estimated_tokens)
    );
    println!(
        "Vocabulary size      : {}",
        format_number(report.vocabulary_size)
    );
    println!(
        "Avg doc length       : {} chars / {} words",
        format_float(report.average_article_length_chars),
        format_float(report.average_article_length_words)
    );
    println!("{}", "=".repeat(48));
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(text: &str) -> XlsumRecord {
        XlsumRecord {
            text: text.into(),
            ..XlsumRecord::default()
        }
    }

    #[test]
    fn counts_documents_chars_and_words() {
        let records = vec![record("waa dal"), record("waa magaalo weyn")];
        let report = compute_stats(&records, TOKENS_PER_WORD).unwrap();
        assert_eq!(report.total_documents, 2);
        assert_eq!(report.total_words, 5);
        assert_eq!(report.total_characters, 23);
    }

    #[test]
    fn vocabulary_is_case_folded_and_deduplicated() {
        let records = vec![record("Dal dal DAL magaalo")];
        let report = compute_stats(&records, TOKENS_PER_WORD).unwrap();
        assert_eq!(report.total_words, 4);
        assert_eq!(report.vocabulary_size, 2);
    }

    #[test]
    fn tokens_scale_with_the_multiplier() {
        let records = vec![record("waa dal wanaagsan oo weyn")];
        let report = compute_stats(&records, 2.0).unwrap();
        assert_eq!(report.estimated_tokens, 10);
    }

    #[test]
    fn empty_corpus_reports_zero_averages() {
        let report = compute_stats(&[], TOKENS_PER_WORD).unwrap();
        assert_eq!(report.total_documents, 0);
        assert_eq!(report.average_article_length_chars, 0.0);
        assert_eq!(report.average_article_length_words, 0.0);
    }

    #[test]
    fn report_is_written_as_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reports/metadata_report.json");
        generate_report(&path, &[record("waa dal")], TOKENS_PER_WORD).unwrap();

        let written: CorpusReport =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written.total_documents, 1);
        assert_eq!(written.config, DATASET_CONFIG);
    }
}
