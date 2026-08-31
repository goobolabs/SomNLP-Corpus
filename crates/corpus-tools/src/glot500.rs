//! cis-lmu/Glot500 downloader.
//!
//! Downloads the `som_Latn` subset of Glot500 from the `refs/convert/parquet`
//! revision on Hugging Face.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use tempfile::NamedTempFile;

use crate::hf::{HfClient, PARQUET_REVISION};
use crate::jsonl::{is_non_empty, JsonlWriter};
use crate::parquet_source::iter_text_column;

pub const SOURCE_TAG: &str = "glot500";
pub const DATASET_NAME: &str = "cis-lmu/Glot500";
pub const DATASET_PREFIX: &str = "som_Latn/train/";

/// Human-readable provenance line for the export summary.
pub fn source_url() -> String {
    format!(
        "https://huggingface.co/datasets/{DATASET_NAME} (prefix: {DATASET_PREFIX}, \
         revision: {PARQUET_REVISION})"
    )
}

/// Download the requested Glot500 som_Latn splits and export one JSONL record per
/// article.
pub fn download_glot500(
    output: &Path,
    limit: Option<u64>,
    streaming: bool,
) -> Result<crate::Stats> {
    let hf = HfClient::new();
    let listed = hf.list_files_at(DATASET_NAME, PARQUET_REVISION, DATASET_PREFIX, true)?;
    let shards = select_shards(&listed)?;

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

fn select_shards(listed: &[String]) -> Result<Vec<String>> {
    let mut shards: Vec<String> = listed
        .iter()
        .filter(|path| path.starts_with(DATASET_PREFIX) && path.ends_with(".parquet"))
        .cloned()
        .collect();

    if shards.is_empty() {
        bail!("no parquet shards found for prefix '{DATASET_PREFIX}'");
    }

    shards.sort();
    Ok(shards)
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

    fn listing() -> Vec<String> {
        vec![
            "som_Latn/test/0000.parquet".to_string(),
            "som_Latn/train/data-00001-of-00002.parquet".to_string(),
            "som_Latn/train/data-00000-of-00002.parquet".to_string(),
            "eng_Latn/train/0000.parquet".to_string(),
            "som_Latn/train/metadata.json".to_string(),
        ]
    }

    #[test]
    fn selects_shards_in_deterministic_order() {
        let shards = select_shards(&listing()).unwrap();
        assert_eq!(
            shards,
            vec![
                "som_Latn/train/data-00000-of-00002.parquet",
                "som_Latn/train/data-00001-of-00002.parquet"
            ]
        );
    }

    #[test]
    fn missing_shards_is_an_error() {
        let empty_listing: Vec<String> = vec![];
        assert!(select_shards(&empty_listing).is_err());

        let wrong_listing = vec!["som_Latn/test/0000.parquet".to_string()];
        assert!(select_shards(&wrong_listing).is_err());
    }

    #[test]
    fn local_shard_names_are_collision_safe() {
        assert_eq!(
            local_shard_name("som_Latn/train/data-00000-of-00002.parquet"),
            "glot500_som_Latn_train_data-00000-of-00002.parquet"
        );
    }
}
