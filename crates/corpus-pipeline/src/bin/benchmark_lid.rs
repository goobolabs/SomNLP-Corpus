//! Benchmark language-ID backends on a labeled eval set, bucketed by length.
//! Decides the LID backend and threshold for the clean/LID stages.
//! See docs/CLEANING_PLAN.md §3.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use corpus_pipeline::config::LidBackend;
use corpus_pipeline::io::{read_jsonl, write_report};
use corpus_pipeline::lid::{self, Detector};
use serde::{Deserialize, Serialize};

const TARGET: &str = "so";
const DEFAULT_EVAL: &str = "crates/corpus-pipeline/tests/fixtures/lid/eval.jsonl";
const DEFAULT_REPORT_JSON: &str = "reports/lid_benchmark.json";
const DEFAULT_REPORT_MD: &str = "reports/lid_benchmark.md";

#[derive(Debug, Parser)]
#[command(about = "Benchmark LID backends (whatlang, lingua) on a labeled eval set")]
struct Args {
    #[arg(long, default_value = DEFAULT_EVAL)]
    eval: PathBuf,

    #[arg(long, default_value = DEFAULT_REPORT_JSON)]
    report_json: PathBuf,

    #[arg(long, default_value = DEFAULT_REPORT_MD)]
    report_md: PathBuf,
}

#[derive(Debug, Deserialize)]
struct EvalRow {
    text: String,
    lang: String,
}

const BUCKETS: &[(&str, usize, usize)] = &[
    ("1-7", 0, 7),
    ("8-15", 8, 15),
    ("16-35", 16, 35),
    ("36+", 36, usize::MAX),
];

fn bucket_label(word_count: usize) -> &'static str {
    BUCKETS
        .iter()
        .find(|(_, lo, hi)| word_count >= *lo && word_count <= *hi)
        .map(|(label, _, _)| *label)
        .unwrap_or("36+")
}

#[derive(Default, Serialize)]
struct Counts {
    n: usize,
    positives: usize,
    tp: usize,
    fp: usize,
    fn_: usize,
}

impl Counts {
    fn recall(&self) -> f64 {
        let denom = self.tp + self.fn_;
        if denom == 0 {
            0.0
        } else {
            self.tp as f64 / denom as f64
        }
    }

    fn precision(&self) -> f64 {
        let denom = self.tp + self.fp;
        if denom == 0 {
            0.0
        } else {
            self.tp as f64 / denom as f64
        }
    }
}

#[derive(Serialize)]
struct BucketResult {
    bucket: String,
    n: usize,
    positives: usize,
    recall: f64,
    precision: f64,
}

#[derive(Serialize)]
struct BackendResult {
    name: String,
    docs_per_sec: f64,
    overall_recall: f64,
    overall_precision: f64,
    by_bucket: Vec<BucketResult>,
}

#[derive(Serialize)]
struct BenchmarkReport {
    generated_at: String,
    eval_path: String,
    eval_size: usize,
    target: String,
    backends: Vec<BackendResult>,
    recommendation: String,
}

fn evaluate(detector: &dyn Detector, rows: &[EvalRow]) -> BackendResult {
    use std::collections::BTreeMap;
    let mut overall = Counts::default();
    let mut by_bucket: BTreeMap<&str, Counts> = BTreeMap::new();

    let start = Instant::now();
    for row in rows {
        let predicted_so = matches!(detector.detect(&row.text), Some((code, _)) if code == TARGET);
        let truly_so = row.lang == TARGET;
        let label = bucket_label(row.text.split_whitespace().count());
        let bucket = by_bucket.entry(label).or_default();

        overall.n += 1;
        bucket.n += 1;
        if truly_so {
            overall.positives += 1;
            bucket.positives += 1;
        }
        match (truly_so, predicted_so) {
            (true, true) => {
                overall.tp += 1;
                bucket.tp += 1;
            }
            (true, false) => {
                overall.fn_ += 1;
                bucket.fn_ += 1;
            }
            (false, true) => {
                overall.fp += 1;
                bucket.fp += 1;
            }
            (false, false) => {}
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    let docs_per_sec = if elapsed > 0.0 {
        rows.len() as f64 / elapsed
    } else {
        f64::INFINITY
    };

    let buckets = BUCKETS
        .iter()
        .filter_map(|(label, _, _)| {
            by_bucket.get(label).map(|c| BucketResult {
                bucket: label.to_string(),
                n: c.n,
                positives: c.positives,
                recall: c.recall(),
                precision: c.precision(),
            })
        })
        .collect();

    BackendResult {
        name: detector.name().to_string(),
        docs_per_sec,
        overall_recall: overall.recall(),
        overall_precision: overall.precision(),
        by_bucket: buckets,
    }
}

fn render_markdown(report: &BenchmarkReport) -> String {
    let mut out = String::new();
    out.push_str("# LID Benchmark\n\n");
    out.push_str(&format!("- Generated: {}\n", report.generated_at));
    out.push_str(&format!(
        "- Eval set: `{}` ({} rows)\n",
        report.eval_path, report.eval_size
    ));
    out.push_str(&format!("- Target language: `{}`\n\n", report.target));
    out.push_str("## Overall\n\n");
    out.push_str("| Backend | Recall | Precision | docs/sec |\n");
    out.push_str("|---|---:|---:|---:|\n");
    for b in &report.backends {
        out.push_str(&format!(
            "| {} | {:.3} | {:.3} | {:.0} |\n",
            b.name, b.overall_recall, b.overall_precision, b.docs_per_sec
        ));
    }
    out.push_str("\n## By length bucket (recall / precision)\n\n");
    for b in &report.backends {
        out.push_str(&format!("\n### {}\n\n", b.name));
        out.push_str("| Bucket (words) | n | positives | Recall | Precision |\n");
        out.push_str("|---|---:|---:|---:|---:|\n");
        for bucket in &b.by_bucket {
            out.push_str(&format!(
                "| {} | {} | {} | {:.3} | {:.3} |\n",
                bucket.bucket, bucket.n, bucket.positives, bucket.recall, bucket.precision
            ));
        }
    }
    out.push_str(&format!(
        "\n## Recommendation\n\n**{}** (highest recall, then precision).\n",
        report.recommendation
    ));
    out
}

fn main() -> Result<()> {
    let args = Args::parse();
    let rows: Vec<EvalRow> = read_jsonl(&args.eval)?.collect::<Result<_>>()?;
    anyhow::ensure!(!rows.is_empty(), "eval set is empty: {}", args.eval.display());

    let detectors: Vec<Box<dyn Detector>> =
        vec![lid::build(LidBackend::Whatlang), lid::build(LidBackend::Lingua)];

    let mut backends: Vec<BackendResult> = detectors
        .iter()
        .map(|d| evaluate(d.as_ref(), &rows))
        .collect();
    backends.sort_by(|a, b| {
        b.overall_recall
            .partial_cmp(&a.overall_recall)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                b.overall_precision
                    .partial_cmp(&a.overall_precision)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    let recommendation = backends
        .first()
        .map(|b| b.name.clone())
        .unwrap_or_default();

    let report = BenchmarkReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        eval_path: args.eval.display().to_string(),
        eval_size: rows.len(),
        target: TARGET.to_string(),
        backends,
        recommendation,
    };

    write_report(&args.report_json, &report)?;
    std::fs::write(&args.report_md, render_markdown(&report))?;

    println!("LID benchmark over {} rows:", report.eval_size);
    for b in &report.backends {
        println!(
            "  {:9} recall={:.3} precision={:.3} ({:.0} docs/sec)",
            b.name, b.overall_recall, b.overall_precision, b.docs_per_sec
        );
    }
    println!("Recommended backend: {}", report.recommendation);
    println!("Reports: {} , {}", args.report_json.display(), args.report_md.display());
    Ok(())
}
