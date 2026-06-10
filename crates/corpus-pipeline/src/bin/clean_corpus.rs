//! Clean stage binary: raw merged JSONL -> cleaned `CorpusRecord` JSONL, with a
//! reject sidecar and stats report. See docs/CLEANING_PLAN.md.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;
use clap::Parser;
use common::reject::reject_exact_duplicate;
use common::types::{ContentHash, DocId, QualityFlag, RecordDisposition};
use corpus_pipeline::clean::{clean_record, CleanResult};
use corpus_pipeline::config::PipelineConfig;
use corpus_pipeline::io::{read_raw, write_report, JsonlSink, RejectWriter};
use corpus_pipeline::progress::{count_jsonl_lines, RecordProgress};
use corpus_pipeline::report::{
    print_banner, print_drops_by_reason, print_kv, print_paths, print_per_source_flow, pct,
    quality_flag_name, write_markdown_companion,
};
use serde::Serialize;

const DEFAULT_INPUT: &str = "data/merged/merged_so.jsonl";
const DEFAULT_OUTPUT: &str = "data/cleaned/cleaned_so.jsonl";
const DEFAULT_CONFIG: &str = "configs/pipeline.toml";
const DEFAULT_REPORT: &str = "reports/02_clean_stats.json";

#[derive(Debug, Parser)]
#[command(about = "Clean merged Somali corpus into processed CorpusRecord JSONL")]
struct Args {
    #[arg(long, default_value = DEFAULT_INPUT)]
    input: PathBuf,

    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    #[arg(long, default_value = DEFAULT_CONFIG)]
    config: PathBuf,

    #[arg(long, default_value = DEFAULT_REPORT)]
    report: PathBuf,

    #[arg(long)]
    limit: Option<u64>,
}

#[derive(Default, Serialize)]
struct CleanReport {
    phase: &'static str,
    generated_at: String,
    config: CleanConfigSnapshot,
    input_docs: u64,
    skipped_unknown_source: u64,
    output_docs: u64,
    review_docs: u64,
    rejected_docs: u64,
    drop_rate: f64,
    drops_by_reason: BTreeMap<String, u64>,
    review_flags: BTreeMap<String, u64>,
    per_source_input: BTreeMap<String, u64>,
    per_source_kept: BTreeMap<String, u64>,
    per_source_rejected: BTreeMap<String, u64>,
    per_source_drops_by_reason: BTreeMap<String, BTreeMap<String, u64>>,
    output_file: String,
    reject_sidecar: String,
}

#[derive(Default, Serialize)]
struct CleanConfigSnapshot {
    document_min_words: usize,
    sentence_min_words: usize,
    ufffd_reject_ratio: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = PipelineConfig::load(&args.config)?;
    let collected_at = Utc::now();

    let mut output = JsonlSink::create(&args.output)?;
    let rejects = RejectWriter::for_output(&args.output);
    let reject_path = rejects.path().to_path_buf();
    let mut rejects = rejects;

    let mut report = CleanReport {
        phase: "02_clean",
        generated_at: collected_at.to_rfc3339(),
        config: CleanConfigSnapshot {
            document_min_words: config.clean.document_min_words,
            sentence_min_words: config.clean.sentence_min_words,
            ufffd_reject_ratio: config.clean.ufffd_reject_ratio,
        },
        output_file: args.output.display().to_string(),
        reject_sidecar: reject_path.display().to_string(),
        ..Default::default()
    };

    let mut seen: HashMap<ContentHash, DocId> = HashMap::new();

    let total = args
        .limit
        .or_else(|| count_jsonl_lines(&args.input));
    eprintln!();
    eprintln!("{}", "─".repeat(56));
    eprintln!("Stage: Clean");
    eprintln!(
        "  input: {}  output: {}",
        args.input.display(),
        args.output.display()
    );
    eprintln!(
        "  min words: document={}, sentence={}",
        config.clean.document_min_words, config.clean.sentence_min_words
    );
    eprintln!("{}", "─".repeat(56));

    let progress = RecordProgress::start("Cleaning records", total);

    for raw in read_raw(&args.input)? {
        if args.limit.is_some_and(|limit| report.input_docs >= limit) {
            break;
        }
        let raw = raw?;
        report.input_docs += 1;
        progress.inc();

        let source_key = raw.source.as_deref().unwrap_or("unknown");
        *report
            .per_source_input
            .entry(source_key.to_string())
            .or_insert(0) += 1;

        let mut record = match clean_record(&raw, &config.clean, collected_at) {
            CleanResult::Skipped => {
                report.skipped_unknown_source += 1;
                continue;
            }
            CleanResult::Processed(record) => *record,
        };

        let source = record.provenance.source.0.clone();

        if record.quality.disposition == RecordDisposition::Rejected {
            let reason = quality_flag_name(
                record.quality.flags.first().unwrap_or(&QualityFlag::TooShort),
            );
            *report.drops_by_reason.entry(reason.clone()).or_insert(0) += 1;
            *report
                .per_source_drops_by_reason
                .entry(source.clone())
                .or_default()
                .entry(reason)
                .or_insert(0) += 1;
            *report.per_source_rejected.entry(source).or_insert(0) += 1;
            report.rejected_docs += 1;
            rejects.write(&record)?;
            continue;
        }

        if let Some(canonical) = seen.get(&record.content_hash) {
            reject_exact_duplicate(&mut record, canonical.clone());
            let reason = "exact_duplicate_after_clean".to_string();
            *report.drops_by_reason.entry(reason.clone()).or_insert(0) += 1;
            *report
                .per_source_drops_by_reason
                .entry(source.clone())
                .or_default()
                .entry(reason)
                .or_insert(0) += 1;
            *report.per_source_rejected.entry(source).or_insert(0) += 1;
            report.rejected_docs += 1;
            rejects.write(&record)?;
            continue;
        }
        seen.insert(record.content_hash.clone(), record.id.clone());

        if record.quality.disposition == RecordDisposition::Review {
            report.review_docs += 1;
            for flag in &record.quality.flags {
                *report
                    .review_flags
                    .entry(quality_flag_name(flag))
                    .or_insert(0) += 1;
            }
        }

        *report.per_source_kept.entry(record.provenance.source.0.clone()).or_insert(0) += 1;
        report.output_docs += 1;
        output.write(&record)?;
    }

    report.drop_rate = if report.input_docs == 0 {
        0.0
    } else {
        report.rejected_docs as f64 / report.input_docs as f64
    };

    progress.finish(format!(
        "kept {}, rejected {}",
        report.output_docs, report.rejected_docs
    ));

    output.finish()?;
    let reject_count = rejects.count();
    rejects.finish()?;
    write_report(&args.report, &report)?;

    print_banner("Clean stage complete");
    print_kv("input", report.input_docs);
    print_kv("kept", format!("{} (review: {})", report.output_docs, report.review_docs));
    print_kv(
        "rejected",
        format!("{} ({})", report.rejected_docs, pct(report.rejected_docs, report.input_docs)),
    );
    print_kv("skipped (no source)", report.skipped_unknown_source);
    print_kv(
        "min words",
        format!(
            "document={}, sentence={}",
            config.clean.document_min_words, config.clean.sentence_min_words
        ),
    );
    print_drops_by_reason(&report.drops_by_reason, report.rejected_docs);
    print_per_source_flow(
        &report.per_source_input,
        &report.per_source_kept,
        Some(&report.per_source_rejected),
    );
    if !report.review_flags.is_empty() {
        println!("  review flags (kept, flagged):");
        for (flag, count) in &report.review_flags {
            println!("    - {flag:<20} {count:>8}");
        }
    }
    print_paths(
        &args.output,
        &args.report,
        if reject_count > 0 {
            Some(&reject_path)
        } else {
            None
        },
    );
    write_markdown_companion(&args.report, &markdown_body(&report, reject_count))?;
    Ok(())
}

fn markdown_body(report: &CleanReport, reject_count: u64) -> String {
    let mut md = String::new();
    md.push_str("# Phase 2 — Clean\n\n");
    md.push_str(&format!("- Generated: {}\n", report.generated_at));
    md.push_str(&format!(
        "- Config: document_min_words={}, sentence_min_words={}\n",
        report.config.document_min_words, report.config.sentence_min_words
    ));
    md.push_str(&format!("- Input: {}\n", report.input_docs));
    md.push_str(&format!(
        "- Kept: {} (review: {})\n",
        report.output_docs, report.review_docs
    ));
    md.push_str(&format!(
        "- Rejected: {} ({:.2}%)\n\n",
        report.rejected_docs,
        report.drop_rate * 100.0
    ));

    md.push_str("## Drops by reason\n\n");
    md.push_str("| Reason | Count |\n|---|---:|\n");
    for (reason, count) in &report.drops_by_reason {
        md.push_str(&format!("| {reason} | {count} |\n"));
    }

    md.push_str("\n## Per source\n\n");
    md.push_str("| Source | Input | Kept | Rejected | Drop % |\n|---|---:|---:|---:|---:|\n");
    for source in report.per_source_input.keys() {
        let input = report.per_source_input.get(source).copied().unwrap_or(0);
        let kept = report.per_source_kept.get(source).copied().unwrap_or(0);
        let rejected = report.per_source_rejected.get(source).copied().unwrap_or(0);
        md.push_str(&format!(
            "| {source} | {input} | {kept} | {rejected} | {:.2}% |\n",
            rejected as f64 / input.max(1) as f64 * 100.0
        ));
    }

    md.push_str(&format!(
        "\nReject sidecar: `{}` ({reject_count} records)\n",
        report.reject_sidecar
    ));
    md.push_str("\nInspect rejects: `jq -r '.quality.flags[0], .text' data/cleaned/cleaned_so.rejected.jsonl | head -40`\n");
    md
}
