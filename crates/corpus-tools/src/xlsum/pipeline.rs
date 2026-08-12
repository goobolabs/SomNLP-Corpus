//! Run every XL-Sum stage end to end: download → inspect → extract → clean →
//! export → stats.

use anyhow::Result;

use crate::xlsum::clean::{clean_records, CleanOptions};
use crate::xlsum::config::{default_splits, Layout};
use crate::xlsum::download::fetch_shards;
use crate::xlsum::export::{export_formats, Format};
use crate::xlsum::extract::{extract_records, save_records};
use crate::xlsum::inspect::inspect_shards;
use crate::xlsum::stats::{generate_report, CorpusReport, TOKENS_PER_WORD};

/// Knobs for a full pipeline run.
#[derive(Debug, Clone)]
pub struct PipelineOptions {
    pub splits: Vec<String>,
    /// Re-download shards that are already cached.
    pub force: bool,
    /// Preview records printed by the inspect stage (0 disables previews).
    pub samples: usize,
    pub limit: Option<u64>,
    pub clean: CleanOptions,
    pub formats: Vec<Format>,
    pub tokens_per_word: f64,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            splits: default_splits(),
            force: false,
            samples: 3,
            limit: None,
            clean: CleanOptions::default(),
            formats: vec![Format::Jsonl, Format::Csv, Format::Txt],
            tokens_per_word: TOKENS_PER_WORD,
        }
    }
}

/// Execute every stage and return the final metadata report.
pub fn run_pipeline(layout: &Layout, options: &PipelineOptions) -> Result<CorpusReport> {
    layout.ensure_dirs()?;

    stage(1, "Download");
    let shards = fetch_shards(layout, &options.splits, options.force)?;

    stage(2, "Inspect");
    inspect_shards(
        &layout.schema_report(),
        &shards,
        options.samples,
        options.splits.first().map_or("train", String::as_str),
    )?;

    stage(3, "Extract");
    let records = extract_records(&shards, options.limit)?;
    save_records(&layout.extracted(), &records)?;

    stage(4, "Clean");
    let (cleaned, _drops) = clean_records(&records, &options.clean);
    save_records(&layout.extracted(), &cleaned)?;

    stage(5, "Export");
    export_formats(layout, &cleaned, &options.formats)?;

    stage(6, "Stats");
    let report = generate_report(&layout.stats_report(), &cleaned, options.tokens_per_word)?;

    println!();
    println!(
        "Pipeline complete. Corpus available under {}",
        layout.root().display()
    );
    Ok(report)
}

fn stage(number: u8, name: &str) {
    println!();
    println!("=== [{number}/6] {name} ===");
}
