//! Pipeline orchestrator: run merge -> clean -> lid -> deep_clean -> near_dedup.
//! See docs/CLEANING_PLAN.md and docs/DATA_PIPELINE.md.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::Parser;
use corpus_pipeline::drop_inspect::{print_inspect_menu, write_inspect_script, INSPECT_SCRIPT};
use corpus_pipeline::progress::{
    print_completed_stages, print_stage_checklist, stage_summary_line, RecordProgress,
};
use corpus_pipeline::report::{print_banner, print_kv};

const DEFAULT_CONFIG: &str = "configs/pipeline.toml";
const ALL_STAGES: &[&str] = &["merge", "clean", "lid", "deep_clean", "near_dedup"];

#[derive(Debug, Parser)]
#[command(about = "Run the Somali corpus processing pipeline end to end")]
struct Args {
    #[arg(long, default_value = DEFAULT_CONFIG)]
    config: PathBuf,

    /// Comma-separated subset of stages to run (merge,clean,lid,deep_clean,near_dedup).
    #[arg(long, value_delimiter = ',')]
    stages: Option<Vec<String>>,

    /// Forwarded to merge, clean, lid, and deep_clean for smoke tests.
    #[arg(long)]
    limit: Option<u64>,
}

fn binary_path(name: &str) -> Result<PathBuf> {
    let dir = std::env::current_exe()
        .context("locating current executable")?
        .parent()
        .context("executable has no parent directory")?
        .to_path_buf();
    Ok(dir.join(name))
}

fn run_stage(name: &str, config: &PathBuf, limit: Option<u64>) -> Result<()> {
    let bin = binary_path(name)?;
    let mut cmd = Command::new(&bin);
    cmd.arg("--config").arg(config);
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    if name != "near_dedup" {
        if let Some(limit) = limit {
            cmd.arg("--limit").arg(limit.to_string());
        }
    }

    let status = cmd
        .status()
        .with_context(|| format!("running {}", bin.display()))?;
    if !status.success() {
        bail!("stage {name} failed with status {status}");
    }
    Ok(())
}

fn stage_binary(stage: &str) -> &'static str {
    match stage {
        "merge" => "merge_corpora",
        "clean" => "clean_corpus",
        "lid" => "lid_verify",
        "deep_clean" => "deep_clean",
        "near_dedup" => "near_dedup",
        _ => unreachable!(),
    }
}

fn print_funnel(stages: &[String]) {
    println!();
    println!("  Funnel (this run):");
    for stage in stages {
        if let Some(line) = stage_summary_line(stage) {
            println!("    • {line}");
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let stages = args
        .stages
        .clone()
        .unwrap_or_else(|| ALL_STAGES.iter().map(|s| s.to_string()).collect());

    for stage in &stages {
        if !ALL_STAGES.contains(&stage.as_str()) {
            bail!("unknown stage '{stage}'; valid: {}", ALL_STAGES.join(", "));
        }
    }

    let pipeline_started = Instant::now();

    print_banner("SomNLP pipeline");
    print_kv("config", args.config.display());
    print_kv("stages", stages.join(" → "));
    if let Some(limit) = args.limit {
        print_kv("limit", format!("{limit} records per stage"));
    }
    let mut completed: Vec<String> = Vec::new();

    for stage in &stages {
        let idx = stages.iter().position(|s| s == stage).unwrap_or(0) + 1;
        print_stage_checklist(&stages, &completed, Some(stage));
        println!();
        println!(
            "▶ Stage {idx}/{}: {} ({})",
            stages.len(),
            stage,
            stage_binary(stage)
        );
        println!();

        let stage_started = Instant::now();
        run_stage(stage_binary(stage), &args.config, args.limit)?;
        completed.push(stage.clone());

        if let Some(summary) = stage_summary_line(stage) {
            println!();
            println!(
                "  ✓ {} finished in {:.1}s — {}",
                stage,
                stage_started.elapsed().as_secs_f64(),
                summary
            );
        }
    }

    let write_progress = RecordProgress::start("Writing inspect script", None);
    write_inspect_script(&stages)?;
    write_progress.finish("inspect script ready");

    print_stage_checklist(&stages, &completed, None);
    print_completed_stages(&stages, &completed);
    print_funnel(&stages);

    print_banner("Pipeline complete");
    print_kv(
        "elapsed",
        format!("{:.1}s", pipeline_started.elapsed().as_secs_f64()),
    );
    print_kv("final output", "data/final/final_so.jsonl");
    print_kv("inspect drops", format!("bash {INSPECT_SCRIPT}"));
    println!();
    print_inspect_menu(&stages);
    Ok(())
}
