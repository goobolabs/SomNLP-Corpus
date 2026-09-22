//! FinePDFs Somali (`HuggingFaceFW/finepdfs`, config `som_Latn`) downloader.
//!
//! The dataset contains PDF-derived text. Parquet representations are available on
//! [`PARQUET_REVISION`], under `som_Latn/train/`.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use tempfile::NamedTempFile;

use crate::hf::{HfClient, PARQUET_REVISION};
use crate::jsonl::{is_non_empty, JsonlWriter};
use crate::parquet_source::iter_text_column;

pub const SOURCE_TAG: &str = "finepdfs";
pub const DATASET_NAME: &str = "HuggingFaceFW/finepdfs";
pub const DATASET_PREFIX: &str = "som_Latn/train";

/// Human-readable provenance line for the export summary.
pub fn source_url() -> String {
    format!(
        "https://huggingface.co/datasets/{DATASET_NAME} (prefix: {DATASET_PREFIX}, \
         revision: {PARQUET_REVISION})"
    )
}

/// Download the Somali FinePDFs parquet shards and export one JSONL record per non-empty text row.
pub fn download_finepdfs(
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
        "FinePDFs Somali export complete",
        &stats,
        output,
        &source_url(),
    );
    Ok(stats)
}

/// Pick the parquet shards belonging strictly to `som_Latn/train/`, sorted deterministically.
fn select_shards(listed: &[String]) -> Result<Vec<String>> {
    let prefix = format!("{DATASET_PREFIX}/");
    let mut matched: Vec<String> = listed
        .iter()
        .filter(|path| path.starts_with(&prefix) && path.ends_with(".parquet"))
        .cloned()
        .collect();

    if matched.is_empty() {
        bail!("no parquet shards found under {DATASET_PREFIX}/");
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

/// Flatten a remote shard path into a unique local filename.
fn local_shard_name(remote_path: &str) -> String {
    let flattened = remote_path.trim_start_matches('/').replace('/', "_");
    format!("{SOURCE_TAG}_{flattened}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing() -> Vec<String> {
        vec![
            "som_Latn/train/0001.parquet".to_string(),
            "som_Latn/train/0000.parquet".to_string(),
            "som_Latn/test/0000.parquet".to_string(),
            "eng_Latn/train/0000.parquet".to_string(),
            "ara_Arab/train/0000.parquet".to_string(),
            "som_Latn/train/metadata.txt".to_string(),
        ]
    }

    #[test]
    fn filters_to_exact_som_latn_train_prefix() {
        let shards = select_shards(&listing()).unwrap();
        assert!(shards.iter().all(|s| s.starts_with("som_Latn/train/")));
        assert!(shards.iter().all(|s| s.ends_with(".parquet")));
    }

    #[test]
    fn excludes_somali_test_split() {
        let shards = select_shards(&listing()).unwrap();
        assert!(!shards.iter().any(|s| s.contains("test")));
    }

    #[test]
    fn excludes_other_language_subsets() {
        let shards = select_shards(&listing()).unwrap();
        assert!(!shards.iter().any(|s| s.contains("eng_Latn")));
        assert!(!shards.iter().any(|s| s.contains("ara_Arab")));
    }

    #[test]
    fn orders_shards_deterministically() {
        let shards = select_shards(&listing()).unwrap();
        assert_eq!(
            shards,
            vec![
                "som_Latn/train/0000.parquet".to_string(),
                "som_Latn/train/0001.parquet".to_string(),
            ]
        );
    }

    #[test]
    fn missing_or_empty_shards_return_error() {
        let empty: Vec<String> = vec![];
        assert!(select_shards(&empty).is_err());

        let non_matching = vec![
            "som_Latn/test/0000.parquet".to_string(),
            "eng_Latn/train/0000.parquet".to_string(),
        ];
        assert!(select_shards(&non_matching).is_err());
    }

    #[test]
    fn local_shard_names_are_collision_safe() {
        assert_eq!(
            local_shard_name("som_Latn/train/0000.parquet"),
            "finepdfs_som_Latn_train_0000.parquet"
        );
        assert_ne!(
            local_shard_name("som_Latn/train/0000.parquet"),
            local_shard_name("som_Latn/train/0001.parquet")
        );
    }
}
