//! LID stage binary: read cleaned CorpusRecords, identify language, gate
//! document-class records and tag sentence-class records. See
//! docs/CLEANING_PLAN.md §3.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use common::types::QualityFlag;
use corpus_pipeline::config::PipelineConfig;
use corpus_pipeline::io::{read_corpus, write_report, JsonlSink, RejectWriter};
use corpus_pipeline::lid::{self, stage};
use corpus_pipeline::progress::{count_jsonl_lines, RecordProgress};
use corpus_pipeline::report::{
    print_banner, print_drops_by_reason, print_kv, print_paths, print_per_source_flow, pct,
    write_markdown_companion,
};
use serde::Serialize;

const DEFAULT_INPUT: &str = "data/cleaned/cleaned_so.jsonl";
const DEFAULT_OUTPUT: &str = "data/lid/lid_so.jsonl";
const DEFAULT_CONFIG: &str = "configs/pipeline.toml";
const DEFAULT_REPORT: &str = "reports/03_lid_stats.json";

#[derive(Debug, Parser)]
#[command(about = "Verify language of cleaned Somali corpus records")]
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
struct LidReport {
    phase: &'static str,
    generated_at: String,
    backend: String,
    min_confidence: f64,
    input_docs: u64,
    output_docs: u64,
    rejected_docs: u64,
    drop_rate: f64,
    drops_by_reason: BTreeMap<String, u64>,
    dropped_lang_counts: BTreeMap<String, u64>,
    per_source_input: BTreeMap<String, u64>,
    per_source_kept: BTreeMap<String, u64>,
    per_source_rejected: BTreeMap<String, u64>,
    output_file: String,
    reject_sidecar: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = PipelineConfig::load(&args.config)?;
    let detector = lid::build(config.lid.backend);

    let mut output = JsonlSink::create(&args.output)?;
    let rejects = RejectWriter::for_output(&args.output);
    let reject_path = rejects.path().to_path_buf();
    let mut rejects = rejects;

    let mut report = LidReport {
        phase: "03_lid",
        generated_at: chrono::Utc::now().to_rfc3339(),
        backend: detector.name().to_string(),
        min_confidence: config.lid.min_confidence,
        output_file: args.output.display().to_string(),
        reject_sidecar: reject_path.display().to_string(),
        ..Default::default()
    };

    let total = args
        .limit
        .or_else(|| count_jsonl_lines(&args.input));
    eprintln!();
    eprintln!("{}", "─".repeat(56));
    eprintln!("Stage: Language identification");
    eprintln!(
        "  input: {}  backend: {}  min_confidence: {}",
        args.input.display(),
        detector.name(),
        config.lid.min_confidence
    );
    eprintln!("{}", "─".repeat(56));

    let progress = RecordProgress::start(
        &format!("LID ({})", detector.name()),
        total,
    );

    for record in read_corpus(&args.input)? {
        if args.limit.is_some_and(|limit| report.input_docs >= limit) {
            break;
        }
        let mut record = record?;
        report.input_docs += 1;
        progress.inc();

        let source = record.provenance.source.0.clone();
        *report.per_source_input.entry(source.clone()).or_insert(0) += 1;

        stage::apply_lid(
            &mut record,
            detector.as_ref(),
            config.lid.min_confidence,
            config.lid.detect_clip_bytes,
        );

        if stage::is_kept(&record) {
            *report.per_source_kept.entry(source).or_insert(0) += 1;
            report.output_docs += 1;
            output.write(&record)?;
        } else {
            let reason = if record.quality.flags.contains(&QualityFlag::NotSomali) {
                "not_somali".to_string()
            } else {
                "low_lang_score".to_string()
            };
            *report.drops_by_reason.entry(reason).or_insert(0) += 1;

            let lang = if record.quality.flags.contains(&QualityFlag::NotSomali) {
                record.provenance.lang.0.clone()
            } else {
                "so_low_conf".to_string()
            };
            *report.dropped_lang_counts.entry(lang).or_insert(0) += 1;
            *report.per_source_rejected.entry(source).or_insert(0) += 1;
            report.rejected_docs += 1;
            rejects.write(&record)?;
        }
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

    print_banner(&format!("LID stage complete ({})", report.backend));
    print_kv("input", report.input_docs);
    print_kv("kept", report.output_docs);
    print_kv(
        "rejected",
        format!("{} ({})", report.rejected_docs, pct(report.rejected_docs, report.input_docs)),
    );
    print_kv("min confidence", report.min_confidence);
    print_drops_by_reason(&report.drops_by_reason, report.rejected_docs);
    if !report.dropped_lang_counts.is_empty() {
        println!("  detected language (rejects):");
        for (lang, count) in &report.dropped_lang_counts {
            println!("    - {lang:<20} {count:>8}");
        }
    }
    print_per_source_flow(
        &report.per_source_input,
        &report.per_source_kept,
        Some(&report.per_source_rejected),
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

fn markdown_body(report: &LidReport, reject_count: u64) -> String {
    let mut md = String::new();
    md.push_str("# Phase 3 — Language identification\n\n");
    md.push_str(&format!("- Generated: {}\n", report.generated_at));
    md.push_str(&format!(
        "- Backend: {} (min_confidence={})\n",
        report.backend, report.min_confidence
    ));
    md.push_str(&format!("- Input: {}\n", report.input_docs));
    md.push_str(&format!("- Kept: {}\n", report.output_docs));
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

    if !report.dropped_lang_counts.is_empty() {
        md.push_str("\n## Detected language (rejects)\n\n");
        md.push_str("| Lang | Count |\n|---|---:|\n");
        for (lang, count) in &report.dropped_lang_counts {
            md.push_str(&format!("| {lang} | {count} |\n"));
        }
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
    md
}
