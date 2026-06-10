//! Near-dedup stage binary: MinHash/LSH near-duplicate removal on document-class
//! records; sentence-class records pass through unchanged. See
//! docs/CLEANING_PLAN.md (Near-deduplication).

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use common::registry;
use common::reject::reject_near_duplicate;
use corpus_pipeline::config::PipelineConfig;
use corpus_pipeline::io::{read_corpus, write_report, JsonlSink, RejectWriter};
use corpus_pipeline::near_dedup::{near_dedup, shingle};
use corpus_pipeline::progress::{count_jsonl_lines, PhaseProgress, RecordProgress};
use corpus_pipeline::report::{
    print_banner, print_kv, print_paths, print_per_source_flow, pct, write_markdown_companion,
};
use serde::Serialize;

const DEFAULT_INPUT: &str = "data/lid/lid_so.jsonl";
const DEFAULT_OUTPUT: &str = "data/final/final_so.jsonl";
const DEFAULT_CONFIG: &str = "configs/pipeline.toml";
const DEFAULT_REPORT: &str = "reports/04_near_dedup_stats.json";

#[derive(Debug, Parser)]
#[command(about = "Near-deduplicate the corpus (document class only) into final output")]
struct Args {
    #[arg(long, default_value = DEFAULT_INPUT)]
    input: PathBuf,

    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    #[arg(long, default_value = DEFAULT_CONFIG)]
    config: PathBuf,

    #[arg(long, default_value = DEFAULT_REPORT)]
    report: PathBuf,
}

#[derive(Default, Serialize)]
struct NearDedupReport {
    phase: &'static str,
    generated_at: String,
    input_docs: u64,
    sentence_passthrough: u64,
    document_input: u64,
    candidate_pairs: usize,
    verified_pairs: usize,
    clusters: usize,
    removed: usize,
    output_docs: u64,
    drop_rate: f64,
    tau: f64,
    drops_by_reason: BTreeMap<String, u64>,
    per_source_input: BTreeMap<String, u64>,
    per_source_kept: BTreeMap<String, u64>,
    per_source_removed: BTreeMap<String, u64>,
    output_file: String,
    reject_sidecar: String,
}

fn is_document_class(source: &str) -> bool {
    registry::lookup(source)
        .map(|entry| entry.near_dedup)
        .unwrap_or(true)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = PipelineConfig::load(&args.config)?;

    let mut output = JsonlSink::create(&args.output)?;
    let rejects = RejectWriter::for_output(&args.output);
    let reject_path = rejects.path().to_path_buf();
    let mut rejects = rejects;

    let mut report = NearDedupReport {
        phase: "04_near_dedup",
        generated_at: chrono::Utc::now().to_rfc3339(),
        tau: config.near_dedup.tau,
        output_file: args.output.display().to_string(),
        reject_sidecar: reject_path.display().to_string(),
        ..Default::default()
    };

    eprintln!();
    eprintln!("{}", "─".repeat(56));
    eprintln!("Stage: Near dedup");
    eprintln!(
        "  input: {}  tau: {}",
        args.input.display(),
        config.near_dedup.tau
    );
    eprintln!("{}", "─".repeat(56));

    let mut phases = PhaseProgress::new(4, "Near dedup");

    phases.next("Reading and partitioning records");
    let total = count_jsonl_lines(&args.input);
    let read_progress = RecordProgress::start("Reading records", total);

    // Document-class records are held in memory for clustering; sentence-class
    // records stream straight through.
    let mut doc_records = Vec::new();
    for record in read_corpus(&args.input)? {
        let record = record?;
        report.input_docs += 1;
        read_progress.inc();

        let source = record.provenance.source.0.clone();
        *report.per_source_input.entry(source.clone()).or_insert(0) += 1;

        if is_document_class(&source) {
            doc_records.push(record);
        } else {
            report.sentence_passthrough += 1;
            *report.per_source_kept.entry(source).or_insert(0) += 1;
            report.output_docs += 1;
            output.write(&record)?;
        }
    }
    report.document_input = doc_records.len() as u64;
    read_progress.finish(format!(
        "{} docs ({} document-class, {} sentence passthrough)",
        report.input_docs, report.document_input, report.sentence_passthrough
    ));

    phases.next("Building shingles");
    let shingle_progress = RecordProgress::start("Shingling documents", Some(report.document_input));
    let shingle_sets: Vec<Vec<u64>> = doc_records
        .iter()
        .map(|r| {
            shingle_progress.inc();
            shingle::shingle_ints(&r.text, config.near_dedup.shingle_k)
        })
        .collect();
    let lengths: Vec<usize> = doc_records.iter().map(|r| r.text.len()).collect();
    shingle_progress.finish("shingles built");

    phases.next("MinHash + LSH + Jaccard clustering");
    let outcome = near_dedup(&shingle_sets, &lengths, &config.near_dedup);
    report.candidate_pairs = outcome.stats.candidate_pairs;
    report.verified_pairs = outcome.stats.verified_pairs;
    report.clusters = outcome.stats.clusters;
    report.removed = outcome.stats.removed;
    report
        .drops_by_reason
        .insert("near_duplicate".to_string(), report.removed as u64);

    phases.next("Writing output and reject sidecar");
    let write_progress = RecordProgress::start("Writing document results", Some(report.document_input));

    // Resolve canonical ids before consuming the records.
    let canonical_ids: Vec<_> = doc_records.iter().map(|r| r.id.clone()).collect();

    for (idx, mut record) in doc_records.into_iter().enumerate() {
        write_progress.inc();
        let source = record.provenance.source.0.clone();
        if let Some(&canonical) = outcome.removed_to_canonical.get(&idx) {
            reject_near_duplicate(&mut record, canonical_ids[canonical].clone());
            *report.per_source_removed.entry(source).or_insert(0) += 1;
            rejects.write(&record)?;
        } else {
            *report.per_source_kept.entry(source).or_insert(0) += 1;
            report.output_docs += 1;
            output.write(&record)?;
        }
    }

    report.drop_rate = if report.input_docs == 0 {
        0.0
    } else {
        report.removed as f64 / report.input_docs as f64
    };

    write_progress.finish(format!("{} near-dups removed", report.removed));
    phases.finish(format!(
        "kept {}, removed {}",
        report.output_docs, report.removed
    ));

    output.finish()?;
    let reject_count = rejects.count();
    rejects.finish()?;
    write_report(&args.report, &report)?;

    print_banner(&format!("Near-dedup stage complete (tau={})", report.tau));
    print_kv("input", report.input_docs);
    print_kv("document class", report.document_input);
    print_kv("sentence passthrough", report.sentence_passthrough);
    print_kv(
        "candidates",
        format!(
            "{} ({} verified >= tau)",
            report.candidate_pairs, report.verified_pairs
        ),
    );
    print_kv("clusters", report.clusters);
    print_kv(
        "removed",
        format!("{} ({})", report.removed, pct(report.removed as u64, report.input_docs)),
    );
    print_kv("kept", report.output_docs);
    print_per_source_flow(
        &report.per_source_input,
        &report.per_source_kept,
        Some(&report.per_source_removed),
    );
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

fn markdown_body(report: &NearDedupReport, reject_count: u64) -> String {
    let mut md = String::new();
    md.push_str("# Phase 4 — Near deduplication\n\n");
    md.push_str(&format!("- Generated: {}\n", report.generated_at));
    md.push_str(&format!("- Tau (Jaccard): {}\n", report.tau));
    md.push_str(&format!("- Input: {}\n", report.input_docs));
    md.push_str(&format!("- Document class: {}\n", report.document_input));
    md.push_str(&format!("- Sentence passthrough: {}\n", report.sentence_passthrough));
    md.push_str(&format!(
        "- Candidate pairs: {} ({} verified)\n",
        report.candidate_pairs, report.verified_pairs
    ));
    md.push_str(&format!("- Clusters: {}\n", report.clusters));
    md.push_str(&format!(
        "- Removed: {} ({:.2}% of input)\n",
        report.removed,
        report.drop_rate * 100.0
    ));
    md.push_str(&format!("- Kept: {}\n\n", report.output_docs));

    md.push_str("## Per source\n\n");
    md.push_str("| Source | Input | Kept | Removed | Drop % |\n|---|---:|---:|---:|---:|\n");
    for source in report.per_source_input.keys() {
        let input = report.per_source_input.get(source).copied().unwrap_or(0);
        let kept = report.per_source_kept.get(source).copied().unwrap_or(0);
        let removed = report.per_source_removed.get(source).copied().unwrap_or(0);
        md.push_str(&format!(
            "| {source} | {input} | {kept} | {removed} | {:.2}% |\n",
            removed as f64 / input.max(1) as f64 * 100.0
        ));
    }

    md.push_str(&format!(
        "\nReject sidecar: `{}` ({reject_count} records)\n",
        report.reject_sidecar
    ));
    md
}
