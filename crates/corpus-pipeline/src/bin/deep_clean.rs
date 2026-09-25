//! Deep-clean stage binary: LID-verified JSONL -> deep-cleaned CorpusRecord JSONL.
//! See docs/CLEANING_STRATEGY.md.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;
use clap::Parser;
use common::reject;
use common::types::{CorpusRecord, RecordDisposition};
use corpus_pipeline::config::PipelineConfig;
use corpus_pipeline::deep_clean::deep_clean_record;
use corpus_pipeline::io::{par_map_jsonl, to_line, write_report, JsonlSink, RejectWriter};
use corpus_pipeline::lid::build;
use corpus_pipeline::progress::{count_jsonl_lines, RecordProgress};
use corpus_pipeline::report::{
    pct, print_banner, print_drops_by_reason, print_kv, print_paths, print_per_source_flow,
    quality_flag_name, write_markdown_companion,
};
use serde::Serialize;

const DEFAULT_INPUT: &str = "data/lid/lid_so.jsonl";
const DEFAULT_OUTPUT: &str = "data/deep_clean/deep_clean_so.jsonl";
const DEFAULT_CONFIG: &str = "configs/pipeline.toml";
const DEFAULT_REPORT: &str = "reports/04_deep_clean_stats.json";

#[derive(Debug, Parser)]
#[command(about = "Deep-clean the Somali corpus (v0.2) into processed CorpusRecord JSONL")]
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
struct DeepCleanReport {
    phase: &'static str,
    generated_at: String,
    input_docs: u64,
    output_docs: u64,
    rejected_docs: u64,
    passthrough_rejected: u64,
    drop_rate: f64,
    drops_by_reason: BTreeMap<String, u64>,
    per_source_input: BTreeMap<String, u64>,
    per_source_kept: BTreeMap<String, u64>,
    per_source_rejected: BTreeMap<String, u64>,
    per_source_drops_by_reason: BTreeMap<String, BTreeMap<String, u64>>,
    output_file: String,
    reject_sidecar: String,
}

/// One deep-cleaned record, already serialized for its destination file.
enum Processed {
    /// Rejected by an earlier stage; copied to the reject sidecar unchanged.
    Passthrough {
        source: String,
        line: String,
    },
    Rejected {
        source: String,
        reason: String,
        line: String,
    },
    Kept {
        source: String,
        line: String,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = PipelineConfig::load(&args.config)?;
    let detector = build(config.lid.backend);

    let mut output = JsonlSink::create(&args.output)?;
    let rejects = RejectWriter::for_output(&args.output);
    let reject_path = rejects.path().to_path_buf();
    let mut rejects = rejects;

    let mut report = DeepCleanReport {
        phase: "04_deep_clean",
        generated_at: Utc::now().to_rfc3339(),
        output_file: args.output.display().to_string(),
        reject_sidecar: reject_path.display().to_string(),
        ..Default::default()
    };

    let total = args.limit.or_else(|| count_jsonl_lines(&args.input));
    eprintln!();
    eprintln!("{}", "─".repeat(56));
    eprintln!("Stage: Deep clean (v0.2)");
    eprintln!(
        "  input: {}  output: {}",
        args.input.display(),
        args.output.display()
    );
    eprintln!("{}", "─".repeat(56));

    let progress = RecordProgress::start("Deep-cleaning records", total);

    // Deep cleaning and serialization run on the rayon pool; stats and writes stay
    // sequential, in input order.
    let process = |record: CorpusRecord| -> Result<Processed> {
        let source = record.provenance.source.0.clone();
        if record.quality.disposition == RecordDisposition::Rejected {
            let line = to_line(&record)?;
            return Ok(Processed::Passthrough { source, line });
        }
        let outcome =
            deep_clean_record(record, &config.deep_clean, &config.clean, detector.as_ref());
        match outcome.reject {
            Some(flag) => {
                let reason = quality_flag_name(&flag);
                let mut rejected = outcome.record;
                reject::reject(&mut rejected, flag);
                let line = to_line(&rejected)?;
                Ok(Processed::Rejected {
                    source,
                    reason,
                    line,
                })
            }
            None => {
                let line = to_line(&outcome.record)?;
                Ok(Processed::Kept { source, line })
            }
        }
    };
    par_map_jsonl(&args.input, args.limit, process, |processed| {
        report.input_docs += 1;
        progress.inc();

        match processed? {
            Processed::Passthrough { source, line } => {
                *report.per_source_input.entry(source).or_insert(0) += 1;
                report.passthrough_rejected += 1;
                report.rejected_docs += 1;
                rejects.write_line(&line)?;
            }
            Processed::Rejected {
                source,
                reason,
                line,
            } => {
                *report.per_source_input.entry(source.clone()).or_insert(0) += 1;
                *report.drops_by_reason.entry(reason.clone()).or_insert(0) += 1;
                *report
                    .per_source_drops_by_reason
                    .entry(source.clone())
                    .or_default()
                    .entry(reason)
                    .or_insert(0) += 1;
                *report.per_source_rejected.entry(source).or_insert(0) += 1;
                report.rejected_docs += 1;
                rejects.write_line(&line)?;
            }
            Processed::Kept { source, line } => {
                *report.per_source_input.entry(source.clone()).or_insert(0) += 1;
                *report.per_source_kept.entry(source).or_insert(0) += 1;
                report.output_docs += 1;
                output.write_line(&line)?;
            }
        }
        Ok(())
    })?;

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

    print_banner("Deep-clean stage complete");
    print_kv("input", report.input_docs);
    print_kv("kept", report.output_docs);
    print_kv(
        "rejected",
        format!(
            "{} ({})",
            report.rejected_docs,
            pct(report.rejected_docs, report.input_docs)
        ),
    );
    print_drops_by_reason(&report.drops_by_reason, report.rejected_docs);
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

fn markdown_body(report: &DeepCleanReport, reject_count: u64) -> String {
    let mut md = String::new();
    md.push_str("# Phase 4 — Deep clean (v0.2)\n\n");
    md.push_str(&format!("- Generated: {}\n", report.generated_at));
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
    md.push_str(&format!(
        "\nReject sidecar: `{}` ({reject_count} records)\n",
        report.reject_sidecar
    ));
    md
}
