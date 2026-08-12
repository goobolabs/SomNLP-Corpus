//! The streaming core: parse → normalize → filter → deduplicate → write.
//!
//! Rows are handled one at a time, so memory use is driven by the optional
//! dedup sets and the optional audit, never by the size of the archive.

use std::collections::HashSet;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::nllb::audit::Audit;
use crate::nllb::export::CorpusWriter;
use crate::nllb::filter::{FilterOptions, Rejection, RejectionCounts};
use crate::nllb::normalize::{normalize_text, pair_digest, text_digest};
use crate::nllb::record::{build_record, parse_raw_line, NllbRecord, RowError};

/// Malformed rows reported individually before the warnings are summarized.
const MAX_ROW_WARNINGS: u64 = 5;

/// What the stream does with each row.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ProcessOptions {
    pub filter: FilterOptions,
    /// Apply conservative Unicode/whitespace normalization to both sides.
    pub normalize: bool,
    /// Drop repeated English–Somali pairs.
    pub deduplicate: bool,
    /// Drop repeated Somali sentences, whatever their English side.
    pub deduplicate_somali: bool,
    /// Stop after this many successfully parsed rows.
    pub max_rows: Option<u64>,
    pub progress: bool,
}

/// What the stream saw.
#[derive(Debug, Default, Clone)]
pub struct ProcessSummary {
    pub total_lines: u64,
    pub malformed: u64,
    pub invalid_numeric: u64,
    pub valid_parsed: u64,
    pub accepted: u64,
    pub rejections: RejectionCounts,
    /// Accepted records, kept only when there is no writer and no audit —
    /// the in-memory mode used by tests.
    pub records: Vec<NllbRecord>,
}

/// Run `lines` through the pipeline, feeding accepted rows to `writer` and
/// every parsed row to `audit`.
pub fn process_stream(
    lines: impl Iterator<Item = Result<String>>,
    options: &ProcessOptions,
    mut writer: Option<&mut CorpusWriter>,
    mut audit: Option<&mut Audit>,
) -> Result<ProcessSummary> {
    let keep_records = writer.is_none() && audit.is_none();
    let mut summary = ProcessSummary::default();

    let mut seen_pairs: HashSet<[u8; 16]> = HashSet::new();
    let mut seen_somali: HashSet<[u8; 16]> = HashSet::new();
    let mut warnings = 0u64;

    let progress = row_progress(options);

    for line in lines {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if options
            .max_rows
            .is_some_and(|max| summary.valid_parsed >= max)
        {
            break;
        }
        summary.total_lines += 1;

        let mut row = match parse_raw_line(&line) {
            Ok(row) => row,
            Err(error) => {
                match error {
                    RowError::Malformed { .. } => {
                        summary.malformed += 1;
                        summary.rejections.record(Rejection::MalformedRow);
                        if let Some(audit) = audit.as_deref_mut() {
                            audit.record_rejection(Rejection::MalformedRow);
                        }
                    }
                    RowError::InvalidNumeric { .. } => {
                        summary.invalid_numeric += 1;
                        summary.rejections.record(Rejection::InvalidNumeric);
                        if let Some(audit) = audit.as_deref_mut() {
                            audit.record_rejection(Rejection::InvalidNumeric);
                        }
                    }
                }
                warnings += 1;
                if warnings <= MAX_ROW_WARNINGS {
                    eprintln!("Skipping line {}: {error}", summary.total_lines);
                    if warnings == MAX_ROW_WARNINGS {
                        eprintln!("Further skipped-row warnings suppressed; see the run summary.");
                    }
                }
                continue;
            }
        };

        summary.valid_parsed += 1;
        let original_english = row.english.clone();
        let original_somali = row.somali.clone();
        if options.normalize {
            row.english = normalize_text(&row.english);
            row.somali = normalize_text(&row.somali);
        }

        if let Some(audit) = audit.as_deref_mut() {
            audit.add_valid(&row, &original_english, &original_somali);
        }
        if let Some(progress) = &progress {
            progress.inc(1);
        }

        if let Some(reason) = options.filter.check(&row) {
            summary.rejections.record(reason);
            if let Some(audit) = audit.as_deref_mut() {
                audit.record_rejection(reason);
            }
            continue;
        }

        if options.deduplicate && !seen_pairs.insert(pair_digest(&row.english, &row.somali)) {
            summary.rejections.record(Rejection::DuplicatePair);
            if let Some(audit) = audit.as_deref_mut() {
                audit.record_rejection(Rejection::DuplicatePair);
            }
            continue;
        }

        if options.deduplicate_somali && !seen_somali.insert(text_digest(&row.somali)) {
            summary.rejections.record(Rejection::DuplicateSomali);
            if let Some(audit) = audit.as_deref_mut() {
                audit.record_rejection(Rejection::DuplicateSomali);
            }
            continue;
        }

        summary.accepted += 1;
        if let Some(writer) = writer.as_deref_mut() {
            writer.write(&row, summary.accepted)?;
        }
        if let Some(audit) = audit.as_deref_mut() {
            audit.record_accepted();
        }
        if keep_records {
            summary.records.push(build_record(&row, summary.accepted));
        }
    }

    if let Some(progress) = progress {
        progress.finish_and_clear();
    }
    if let Some(audit) = audit {
        audit.record_totals(summary.total_lines, summary.malformed, summary.invalid_numeric);
    }

    Ok(summary)
}

fn row_progress(options: &ProcessOptions) -> Option<ProgressBar> {
    if !options.progress {
        return None;
    }

    let progress = match options.max_rows {
        Some(max) => {
            let bar = ProgressBar::new(max);
            if let Ok(style) =
                ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {pos}/{len} rows")
            {
                bar.set_style(style.progress_chars("=>-"));
            }
            bar
        }
        None => {
            let spinner = ProgressBar::new_spinner();
            if let Ok(style) = ProgressStyle::with_template("{spinner:.green} {msg} {pos} rows") {
                spinner.set_style(style.tick_strings(&[
                    "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
                ]));
            }
            spinner
        }
    };
    progress.set_draw_target(ProgressDrawTarget::stderr());
    progress.set_message("Processing".to_string());
    Some(progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(english: &str, somali: &str) -> String {
        row(english, somali, "1.2000", "0.99", "0.98")
    }

    fn row(english: &str, somali: &str, laser: &str, eng_lid: &str, som_lid: &str) -> String {
        [
            english, somali, laser, eng_lid, som_lid, "src-en", "_", "src-so", "_",
        ]
        .join("\t")
    }

    fn run(lines: Vec<String>, options: ProcessOptions) -> ProcessSummary {
        process_stream(lines.into_iter().map(Ok), &options, None, None).unwrap()
    }

    fn defaults() -> ProcessOptions {
        ProcessOptions::default()
    }

    #[test]
    fn malformed_rows_are_skipped_not_fatal() {
        let summary = run(
            vec![raw("hello", "salaan"), "bad\trow".into(), raw("second", "labaad")],
            defaults(),
        );
        assert_eq!(summary.malformed, 1);
        assert_eq!(summary.valid_parsed, 2);
        assert_eq!(summary.accepted, 2);
        assert_eq!(summary.rejections.get(Rejection::MalformedRow), 1);
    }

    #[test]
    fn unparsable_numbers_are_skipped_not_fatal() {
        let summary = run(
            vec![
                row("hello", "salaan", "NaNsense", "0.99", "0.98"),
                raw("hello", "salaan"),
            ],
            defaults(),
        );
        assert_eq!(summary.invalid_numeric, 1);
        assert_eq!(summary.accepted, 1);
    }

    #[test]
    fn empty_sides_are_rejected_by_default() {
        let summary = run(
            vec![raw("", "salaan"), raw("hello", ""), raw("hello", "salaan")],
            defaults(),
        );
        assert_eq!(summary.rejections.get(Rejection::EmptyEnglish), 1);
        assert_eq!(summary.rejections.get(Rejection::EmptySomali), 1);
        assert_eq!(summary.accepted, 1);
    }

    #[test]
    fn blank_lines_are_not_counted() {
        let summary = run(vec![String::new(), "   ".into(), raw("hello", "salaan")], defaults());
        assert_eq!(summary.total_lines, 1);
        assert_eq!(summary.accepted, 1);
    }

    #[test]
    fn max_rows_stops_after_the_requested_valid_rows() {
        let lines: Vec<String> = (0..10).map(|index| raw(&format!("e{index}"), "salaan")).collect();
        let summary = run(
            lines,
            ProcessOptions {
                max_rows: Some(3),
                ..defaults()
            },
        );
        assert_eq!(summary.valid_parsed, 3);
        assert_eq!(summary.accepted, 3);
    }

    #[test]
    fn thresholds_reject_low_scoring_rows() {
        let summary = run(
            vec![
                row("hello", "salaan", "0.5", "0.99", "0.98"),
                row("hello", "salaan", "1.5", "0.99", "0.98"),
            ],
            ProcessOptions {
                filter: FilterOptions {
                    min_laser: Some(1.0),
                    ..FilterOptions::default()
                },
                ..defaults()
            },
        );
        assert_eq!(summary.accepted, 1);
        assert_eq!(summary.rejections.get(Rejection::LaserTooLow), 1);
    }

    #[test]
    fn pair_dedup_keeps_the_first_of_each_pair() {
        let summary = run(
            vec![raw("A", "B"), raw("A", "B"), raw("A", "C")],
            ProcessOptions {
                deduplicate: true,
                ..defaults()
            },
        );
        assert_eq!(summary.accepted, 2);
        assert_eq!(summary.rejections.get(Rejection::DuplicatePair), 1);
    }

    #[test]
    fn somali_dedup_ignores_the_english_side() {
        let summary = run(
            vec![raw("one", "isku mid"), raw("two", "isku mid")],
            ProcessOptions {
                deduplicate_somali: true,
                ..defaults()
            },
        );
        assert_eq!(summary.accepted, 1);
        assert_eq!(summary.rejections.get(Rejection::DuplicateSomali), 1);
    }

    #[test]
    fn normalization_is_off_unless_requested() {
        let summary = run(vec![raw("  hello   world  ", "salaan")], defaults());
        assert_eq!(summary.records[0].english, "  hello   world  ");

        let summary = run(
            vec![raw("  hello   world  ", "salaan")],
            ProcessOptions {
                normalize: true,
                ..defaults()
            },
        );
        assert_eq!(summary.records[0].english, "hello world");
    }

    #[test]
    fn accepted_records_are_numbered_from_one() {
        let summary = run(vec![raw("a", "b"), raw("", "c"), raw("d", "e")], defaults());
        let ids: Vec<&str> = summary.records.iter().map(|record| record.id.as_str()).collect();
        assert_eq!(ids, vec!["nllb-eng-som-00000001", "nllb-eng-som-00000002"]);
    }

    #[test]
    fn the_audit_sees_rejected_rows_too() {
        let mut audit = Audit::new(0).unwrap();
        let summary = process_stream(
            vec![raw("hello", "salaan"), raw("", "salaan")]
                .into_iter()
                .map(Ok),
            &defaults(),
            None,
            Some(&mut audit),
        )
        .unwrap();

        assert_eq!(summary.accepted, 1);
        // Records are not buffered once an audit or writer is attached.
        assert!(summary.records.is_empty());

        let report = audit.to_report(None);
        assert_eq!(report.valid_parsed_rows, 2);
        assert_eq!(report.accepted_rows, 1);
        assert_eq!(report.total_lines_inspected, 2);
        assert_eq!(report.rejected_rows_by_reason.get("empty English"), Some(1));
    }
}
