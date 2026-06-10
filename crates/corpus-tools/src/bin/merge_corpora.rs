use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::Parser;
use common::hash::content_hash;
use corpus_tools::cli;
use corpus_tools::jsonl::{is_non_empty, read_jsonl_texts, DroppedWriter, JsonlWriter};
use corpus_tools::stats::{format_number, DedupCounters};
use serde::Deserialize;
use serde_json::json;

const DEFAULT_RAW_DIR: &str = "data/raw";
const DEFAULT_OUTPUT: &str = "data/merged/merged_so.jsonl";
const DEFAULT_CONFIG: &str = "configs/pipeline.toml";
const DEFAULT_REPORT: &str = "reports/01_merge_stats.json";
const FALLBACK_ORDER: &[&str] = &["mt560", "opus", "cc100", "mc4", "madlad", "hplt"];

#[derive(Debug, Parser)]
#[command(about = "Merge raw Somali JSONL corpora with streaming exact dedup")]
struct Args {
    #[arg(long, default_value = DEFAULT_RAW_DIR)]
    raw_dir: PathBuf,

    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    #[arg(long, default_value = DEFAULT_CONFIG)]
    config: PathBuf,

    #[arg(long, default_value = DEFAULT_REPORT)]
    report: PathBuf,

    /// Override the source order. Defaults to `merge_source_order` from config.
    #[arg(long, num_args = 1..)]
    sources: Option<Vec<String>>,

    #[arg(long)]
    limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct MergeOrderConfig {
    merge_source_order: Vec<String>,
}

fn resolve_sources(args: &Args) -> Vec<String> {
    if let Some(sources) = &args.sources {
        return sources.clone();
    }
    match std::fs::read_to_string(&args.config) {
        Ok(text) => match toml::from_str::<MergeOrderConfig>(&text) {
            Ok(config) => config.merge_source_order,
            Err(error) => {
                eprintln!(
                    "Warning: could not parse {} ({error}); using fallback order",
                    args.config.display()
                );
                FALLBACK_ORDER.iter().map(|s| s.to_string()).collect()
            }
        },
        Err(_) => FALLBACK_ORDER.iter().map(|s| s.to_string()).collect(),
    }
}

fn source_path(raw_dir: &Path, source: &str) -> PathBuf {
    raw_dir.join(source).join(format!("{source}_so.jsonl"))
}

fn pct(n: u64, total: u64) -> String {
    if total == 0 {
        "0.00%".to_string()
    } else {
        format!("{:.2}%", n as f64 / total as f64 * 100.0)
    }
}

fn write_markdown_report(
    json_path: &Path,
    counters: &DedupCounters,
    output: &Path,
    source_status: &[(String, bool, u64)],
) -> Result<()> {
    let md_path = json_path.with_extension("md");
    if let Some(parent) = md_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut md = String::new();
    md.push_str("# Phase 1 — Merge + exact dedup\n\n");
    md.push_str(&format!(
        "- Input: {}\n",
        format_number(counters.total_input)
    ));
    md.push_str(&format!(
        "- Kept: {}\n",
        format_number(counters.total_kept)
    ));
    md.push_str(&format!(
        "- Dropped: {} ({:.2}%)\n",
        format_number(counters.total_dropped()),
        counters.total_dropped() as f64 / counters.total_input.max(1) as f64 * 100.0
    ));
    md.push_str(&format!(
        "- Within-source dups: {}\n",
        counters.within_source_dups.values().sum::<u64>()
    ));
    md.push_str(&format!(
        "- Cross-source dups: {}\n\n",
        counters.cross_source_dups.values().sum::<u64>()
    ));

    md.push_str("## Per source\n\n");
    md.push_str("| Source | Status | Input | Kept | Dropped | Drop % |\n|---|---|---:|---:|---:|---:|\n");
    for source in counters.per_source_input.keys() {
        let input = counters.per_source_input.get(source).copied().unwrap_or(0);
        let kept = counters.per_source_kept.get(source).copied().unwrap_or(0);
        let dropped = input.saturating_sub(kept);
        let status = source_status
            .iter()
            .find(|(s, _, _)| s == source)
            .map(|(_, exists, _)| if *exists { "found" } else { "missing" })
            .unwrap_or("found");
        md.push_str(&format!(
            "| {source} | {status} | {input} | {kept} | {dropped} | {:.2}% |\n",
            dropped as f64 / input.max(1) as f64 * 100.0
        ));
    }

    md.push_str(&format!("\nOutput: `{}`\n", output.display()));
    md.push_str("\n## View dropped texts\n\n");
    md.push_str("```bash\n");
    md.push_str("jq -r '[.reason, .source, .text] | @tsv' data/merged/merged_so.dropped.jsonl | head -30\n");
    md.push_str("bash reports/inspect_drops.sh merge\n");
    md.push_str("```\n");
    std::fs::write(&md_path, md)?;
    println!("  report (md)         : {}", md_path.display());
    Ok(())
}

fn write_report(
    path: &Path,
    counters: &DedupCounters,
    output: &Path,
    dropped_path: &Path,
    dropped_count: u64,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    let report = json!({
        "phase": "01_merge",
        "generated_at": Utc::now().to_rfc3339(),
        "total_input_docs": counters.total_input,
        "total_output_docs": counters.total_kept,
        "total_dropped": counters.total_dropped(),
        "drop_rate": if counters.total_input == 0 {
            0.0
        } else {
            counters.total_dropped() as f64 / counters.total_input as f64
        },
        "within_source_dup_drops": counters.within_source_dups,
        "cross_source_dup_drops": counters.cross_source_dups,
        "per_source_input": counters.per_source_input,
        "per_source_kept": counters.per_source_kept,
        "output_file": output.display().to_string(),
        "dropped_sidecar": dropped_path.display().to_string(),
        "dropped_count": dropped_count,
    });
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(path, json).with_context(|| format!("writing report {}", path.display()))?;
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let sources = resolve_sources(&args);

    eprintln!();
    eprintln!("{}", "─".repeat(56));
    eprintln!("Stage: Merge + exact dedup");
    eprintln!("  sources: {}", sources.join(" → "));
    eprintln!("  output:  {}", args.output.display());
    if let Some(limit) = args.limit {
        eprintln!("  limit:   {limit} kept records");
    }
    eprintln!("{}", "─".repeat(56));

    let mut writer = JsonlWriter::create(&args.output, "Merging")?;
    let mut dropped = DroppedWriter::for_output(&args.output);
    let dropped_path = dropped.path().to_path_buf();
    let mut counters = DedupCounters::default();
    // content hash -> source key of the first record that carried it.
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut source_status = Vec::new();

    'sources: for source in &sources {
        let path = source_path(&args.raw_dir, source);
        if !path.exists() {
            eprintln!("Skipping missing source: {}", path.display());
            source_status.push((source.clone(), false, 0u64));
            continue;
        }

        let mut kept_here = 0u64;
        for text in read_jsonl_texts(&path)? {
            if args.limit.is_some_and(|limit| counters.total_kept >= limit) {
                source_status.push((source.clone(), true, kept_here));
                break 'sources;
            }
            let text = text?;
            if !is_non_empty(&text) {
                continue;
            }

            counters.record_input(source);
            let hash = content_hash(&text).0;
            if let Some(first_source) = seen.get(&hash) {
                if first_source == source {
                    counters.record_within_dup(source);
                    dropped.write(source, &text, "within_source_dup", None)?;
                } else {
                    counters.record_cross_dup(source);
                    dropped.write(source, &text, "cross_source_dup", Some(first_source))?;
                }
                continue;
            }

            seen.insert(hash, source.clone());
            writer.write_text_source(source, &text)?;
            counters.record_kept(source);
            kept_here += 1;
        }

        source_status.push((source.clone(), true, kept_here));
    }

    if writer.stats.total_docs == 0 {
        bail!("No documents merged. Download raw sources first or check --sources.");
    }

    let stats = writer.stats.clone();
    writer.finish();
    let dropped_count = dropped.count;
    dropped.finish()?;
    write_report(
        &args.report,
        &counters,
        &args.output,
        &dropped_path,
        dropped_count,
    )?;
    cli::print_merge_summary(&stats, &args.output, &source_status);

    println!();
    println!("{}", "=".repeat(56));
    println!("Exact dedup summary");
    println!("{}", "=".repeat(56));
    println!(
        "  {:<22} {}",
        "input",
        format_number(counters.total_input)
    );
    println!(
        "  {:<22} {}",
        "kept",
        format_number(counters.total_kept)
    );
    println!(
        "  {:<22} {} ({})",
        "dropped",
        format_number(counters.total_dropped()),
        pct(counters.total_dropped(), counters.total_input)
    );
    let within: u64 = counters.within_source_dups.values().sum();
    let cross: u64 = counters.cross_source_dups.values().sum();
    println!("  drops by reason:");
    println!(
        "    - {:<20} {:>8}  ({})",
        "within_source_dup",
        within,
        pct(within, counters.total_dropped())
    );
    println!(
        "    - {:<20} {:>8}  ({})",
        "cross_source_dup",
        cross,
        pct(cross, counters.total_dropped())
    );
    println!("  per source:");
    println!(
        "    {:<10} {:>10} {:>10} {:>10} {:>8}",
        "source", "input", "kept", "dropped", "drop%"
    );
    for source in counters.per_source_input.keys() {
        let input = counters.per_source_input.get(source).copied().unwrap_or(0);
        let kept = counters.per_source_kept.get(source).copied().unwrap_or(0);
        let dropped = input.saturating_sub(kept);
        println!(
            "    {:<10} {:>10} {:>10} {:>10} {:>8}",
            source,
            input,
            kept,
            dropped,
            pct(dropped, input)
        );
    }
    println!("  {:<22} {}", "output", args.output.display());
    println!("  {:<22} {}", "report (json)", args.report.display());
    if dropped_count > 0 {
        println!("  {:<22} {}", "dropped sidecar", dropped_path.display());
        println!("  view dropped texts:");
        println!(
            "    jq -r '[.reason, .source, .text] | @tsv' {} | head -30",
            dropped_path.display()
        );
        println!("    bash reports/inspect_drops.sh merge");
    }
    write_markdown_report(&args.report, &counters, &args.output, &source_status)?;
    Ok(())
}
