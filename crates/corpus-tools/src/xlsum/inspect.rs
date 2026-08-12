//! Stage 2 — report the schema, per-split row counts, and a few samples.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::parquet_source::{column_types, iter_string_rows, row_count};
use crate::xlsum::config::{COLUMNS, DATASET_CONFIG, DATASET_NAME};
use crate::xlsum::download::Shard;

/// Schema summary of the downloaded shards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaReport {
    pub repo: String,
    pub config: String,
    pub columns: Vec<String>,
    pub features: BTreeMap<String, String>,
    pub splits: BTreeMap<String, u64>,
    pub total_rows: u64,
}

/// Build the schema report from the shards' parquet footers.
pub fn build_schema_report(shards: &[Shard]) -> Result<SchemaReport> {
    let first = shards
        .first()
        .context("no shards to inspect; run the download stage first")?;

    let types = column_types(&first.local_path)?;
    let columns: Vec<String> = types.iter().map(|(name, _)| name.clone()).collect();
    let features: BTreeMap<String, String> = types.into_iter().collect();

    let mut splits: BTreeMap<String, u64> = BTreeMap::new();
    for shard in shards {
        *splits.entry(shard.split.clone()).or_insert(0) += row_count(&shard.local_path)?;
    }
    let total_rows = splits.values().sum();

    Ok(SchemaReport {
        repo: DATASET_NAME.to_string(),
        config: DATASET_CONFIG.to_string(),
        columns,
        features,
        splits,
        total_rows,
    })
}

/// Print the schema plus `samples` preview records, and write the report.
pub fn inspect_shards(
    report_path: &Path,
    shards: &[Shard],
    samples: usize,
    preview_split: &str,
) -> Result<SchemaReport> {
    let report = build_schema_report(shards)?;

    println!("Repository : {} ({})", report.repo, report.config);
    println!("Columns    : {}", report.columns.join(", "));
    println!("Total rows : {}", report.total_rows);
    for (split, rows) in &report.splits {
        println!("  {split:<11} {rows} row(s)");
    }
    println!("Feature schema:");
    for (name, feature) in &report.features {
        println!("  - {name:<8} : {feature}");
    }

    if samples > 0 {
        print_samples(shards, samples, preview_split)?;
    }

    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    std::fs::write(report_path, serde_json::to_string_pretty(&report)? + "\n")
        .with_context(|| format!("writing {}", report_path.display()))?;
    println!("Schema report -> {}", report_path.display());

    Ok(report)
}

fn print_samples(shards: &[Shard], samples: usize, preview_split: &str) -> Result<()> {
    let shard = shards
        .iter()
        .find(|shard| shard.split == preview_split)
        .or_else(|| shards.first())
        .context("no shards available to preview")?;

    println!("Previewing {samples} record(s) from '{}':", shard.split);
    let columns: Vec<String> = COLUMNS.iter().map(|name| name.to_string()).collect();
    for (index, row) in iter_string_rows(&shard.local_path, &columns)?
        .take(samples)
        .enumerate()
    {
        let row = row?;
        println!("  [{index}] title  : {}", preview(&row[2]));
        println!("      summary: {}", preview(&row[4]));
        println!("      text   : {}", preview(&row[3]));
    }
    Ok(())
}

/// Single-line, truncated preview of a value.
fn preview(value: &str) -> String {
    const WIDTH: usize = 90;
    let flattened: String = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if flattened.chars().count() <= WIDTH {
        return flattened;
    }
    let head: String = flattened.chars().take(WIDTH).collect();
    format!("{head}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_flattens_whitespace() {
        assert_eq!(
            preview("Suudaan   oo\nbaadigoobaysa"),
            "Suudaan oo baadigoobaysa"
        );
    }

    #[test]
    fn preview_truncates_long_values_with_an_ellipsis() {
        let long = "a".repeat(200);
        let shortened = preview(&long);
        assert_eq!(shortened.chars().count(), 91);
        assert!(shortened.ends_with('…'));
    }

    #[test]
    fn schema_report_needs_at_least_one_shard() {
        assert!(build_schema_report(&[]).is_err());
    }
}
