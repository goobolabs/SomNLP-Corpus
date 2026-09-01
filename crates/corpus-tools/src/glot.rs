//! Glot500 Somali (`cis-lmu/Glot500`, config `som_Latn`) downloader.
//!
//! The Hub auto-converts script-based configs to parquet on [`PARQUET_REVISION`].

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use tempfile::NamedTempFile;

use crate::hf::{HfClient, PARQUET_REVISION};
use crate::jsonl::{is_non_empty, JsonlWriter};
use crate::parquet_source::iter_text_column;

pub const SOURCE_TAG: &str = "glot";
pub const DATASET_NAME: &str = "cis-lmu/Glot500";
pub const DATASET_CONFIG: &str = "som_Latn";
pub const DEFAULT_SPLIT: &str = "train";

pub fn source_url() -> String {
    format!(
        "https://huggingface.co/datasets/{DATASET_NAME} (config: {DATASET_CONFIG}, \
         revision: {PARQUET_REVISION})"
    )
}

/// Download the Somali Latin Glot500 split and export one JSONL record per row.
pub fn download_glot(
    output: &Path,
    split: &str,
    limit: Option<u64>,
    streaming: bool,
) -> Result<crate::Stats> {
    let hf = HfClient::new();
    let listed = hf.list_files_at(DATASET_NAME, PARQUET_REVISION, DATASET_CONFIG, true)?;
    let shards = select_shards(&listed, split)?;

    let mut writer = JsonlWriter::create(output, "Writing")?;
    let mut written = 0u64;
    let mut temps: Vec<NamedTempFile> = Vec::new();

    for remote_path in &shards {
        if limit.is_some_and(|limit| written >= limit) {
            break;
        }

        let local_path = resolve_shard(&hf, remote_path, output, streaming, &mut temps)?;

        for text in iter_text_column(&local_path, "text")? {
            if limit.is_some_and(|limit| written >= limit) {
                break;
            }
            let text = text?;
            if !is_non_empty(&text) {
                continue;
            }
            writer.write_text(&text)?;
            written += 1;
        }
    }

    let stats = writer.stats.clone();
    writer.finish();

    if stats.total_docs == 0 {
        bail!("no documents exported from {DATASET_NAME}");
    }

    crate::cli::print_export_summary(
        "Glot500 Somali export complete",
        &stats,
        output,
        &source_url(),
    );
    Ok(stats)
}

fn select_shards(listed: &[String], split: &str) -> Result<Vec<String>> {
    let prefix = format!("{DATASET_CONFIG}/{split}/");
    let mut matched: Vec<String> = listed
        .iter()
        .filter(|path| path.starts_with(&prefix) && path.ends_with(".parquet"))
        .cloned()
        .collect();
    if matched.is_empty() {
        bail!("no parquet shards found for split '{split}' under {DATASET_CONFIG}/");
    }
    matched.sort();
    Ok(matched)
}

fn resolve_shard(
    hf: &HfClient,
    remote_path: &str,
    output: &Path,
    streaming: bool,
    temps: &mut Vec<NamedTempFile>,
) -> Result<PathBuf> {
    if streaming {
        let (temp, path) = hf.download_to_temp_at(DATASET_NAME, PARQUET_REVISION, remote_path)?;
        temps.push(temp);
        return Ok(path);
    }

    let path = output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(local_shard_name(remote_path));
    hf.download_to_path_at(DATASET_NAME, PARQUET_REVISION, remote_path, &path)?;
    Ok(path)
}

fn local_shard_name(remote_path: &str) -> String {
    let flattened = remote_path.trim_start_matches('/').replace('/', "_");
    format!("{SOURCE_TAG}_{flattened}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_train_shards_in_order() {
        let listed = vec![
            "som_Latn/train/0001.parquet".to_string(),
            "som_Latn/train/0000.parquet".to_string(),
            "som_Latn/test/0000.parquet".to_string(),
        ];
        let shards = select_shards(&listed, "train").unwrap();
        assert_eq!(
            shards,
            vec!["som_Latn/train/0000.parquet", "som_Latn/train/0001.parquet"]
        );
    }

    #[test]
    fn missing_split_is_an_error() {
        let listed = vec!["som_Latn/train/0000.parquet".to_string()];
        assert!(select_shards(&listed, "validation").is_err());
    }

    #[test]
    fn local_shard_names_are_unique() {
        assert_ne!(
            local_shard_name("som_Latn/train/0000.parquet"),
            local_shard_name("som_Latn/train/0001.parquet")
        );
    }
}
