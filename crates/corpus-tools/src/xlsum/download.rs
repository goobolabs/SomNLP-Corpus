//! Stage 1 — fetch the Somali XL-Sum parquet shards from the Hub.
//!
//! The repo still ships a (now unsupported) dataset *script*, so the config is
//! not loadable by name. The Hub auto-converts such datasets to parquet on
//! [`PARQUET_REVISION`], which we read instead — one `0000.parquet` shard per
//! split, holding `id` / `url` / `title` / `summary` / `text`.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use tempfile::NamedTempFile;

use crate::hf::{HfClient, PARQUET_REVISION};
use crate::jsonl::{is_non_empty, JsonlWriter};
use crate::parquet_source::iter_text_column;
use crate::xlsum::config::{
    source_url, Layout, DATASET_CONFIG, DATASET_NAME, FIELDS, SOURCE_TAG, SPLITS,
};

/// A parquet shard on disk, tagged with the split it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shard {
    pub split: String,
    pub remote_path: String,
    pub local_path: PathBuf,
}

/// Download the requested Somali XL-Sum splits and export one JSONL record per
/// article, carrying the chosen `field` as `text`.
///
/// No cleaning happens here: like every other downloader in this crate, the
/// output is raw source text and normalization is the corpus-pipeline's job.
/// For the cleaned, multi-format corpus see [`crate::xlsum::pipeline`].
pub fn download_xlsum(
    output: &Path,
    field: &str,
    splits: &[String],
    limit: Option<u64>,
    streaming: bool,
) -> Result<crate::Stats> {
    if !FIELDS.contains(&field) {
        bail!(
            "unknown field '{field}'; expected one of {}",
            FIELDS.join(", ")
        );
    }

    let hf = HfClient::new();
    let listed = hf.list_files_at(DATASET_NAME, PARQUET_REVISION, DATASET_CONFIG, true)?;
    let shards = select_shards(&listed, splits)?;

    let mut writer = JsonlWriter::create(output, "Writing")?;
    let mut written = 0u64;
    let mut temps: Vec<NamedTempFile> = Vec::new();

    for (_, remote_path) in &shards {
        if limit.is_some_and(|limit| written >= limit) {
            break;
        }

        let local_path = resolve_shard(&hf, remote_path, output, streaming, &mut temps)?;

        for text in iter_text_column(&local_path, field)? {
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
        "XL-Sum Somali export complete",
        &stats,
        output,
        &source_url(),
    );
    Ok(stats)
}

/// Materialize every requested split's shards under `layout.raw_dir()`.
///
/// Shards already on disk are reused unless `force` is set, so re-running the
/// staged pipeline does not re-download the dataset.
pub fn fetch_shards(layout: &Layout, splits: &[String], force: bool) -> Result<Vec<Shard>> {
    layout.ensure_dirs()?;

    let hf = HfClient::new();
    let listed = hf.list_files_at(DATASET_NAME, PARQUET_REVISION, DATASET_CONFIG, true)?;
    let selected = select_shards(&listed, splits)?;

    let raw_dir = layout.raw_dir();
    let mut shards = Vec::with_capacity(selected.len());

    for (split, remote_path) in selected {
        let local_path = raw_dir.join(local_shard_name(&remote_path));
        if local_path.exists() && !force {
            println!("  {split:<11} cached  {}", local_path.display());
        } else {
            println!("  {split:<11} -> {remote_path}");
            hf.download_to_path_at(DATASET_NAME, PARQUET_REVISION, &remote_path, &local_path)?;
        }
        shards.push(Shard {
            split,
            remote_path,
            local_path,
        });
    }

    Ok(shards)
}

/// Pick the parquet shards belonging to `splits`, in the order requested.
///
/// Errors when a requested split has no shards, so a renamed split upstream
/// fails loudly instead of silently exporting a short corpus.
fn select_shards(listed: &[String], splits: &[String]) -> Result<Vec<(String, String)>> {
    if splits.is_empty() {
        bail!(
            "no splits requested; expected one or more of {}",
            SPLITS.join(", ")
        );
    }

    let mut shards = Vec::new();
    for split in splits {
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
        shards.extend(matched.into_iter().map(|path| (split.clone(), path)));
    }
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

/// Flatten a remote shard path into a unique local filename.
///
/// Every split names its shard `0000.parquet`, so keeping only the file name
/// would make the three splits overwrite each other on disk.
fn local_shard_name(remote_path: &str) -> String {
    let flattened = remote_path.trim_start_matches('/').replace('/', "_");
    format!("{SOURCE_TAG}_{flattened}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing() -> Vec<String> {
        vec![
            "somali/test/0000.parquet".to_string(),
            "somali/train/0000.parquet".to_string(),
            "somali/validation/0000.parquet".to_string(),
        ]
    }

    fn splits(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn paths(shards: &[(String, String)]) -> Vec<&str> {
        shards.iter().map(|(_, path)| path.as_str()).collect()
    }

    #[test]
    fn selects_shards_in_requested_order() {
        let shards = select_shards(&listing(), &splits(&["train", "test"])).unwrap();
        assert_eq!(
            paths(&shards),
            vec!["somali/train/0000.parquet", "somali/test/0000.parquet"]
        );
    }

    #[test]
    fn tags_each_shard_with_its_split() {
        let shards = select_shards(&listing(), &splits(&["validation"])).unwrap();
        assert_eq!(shards[0].0, "validation");
    }

    #[test]
    fn split_prefix_does_not_match_by_substring() {
        // "valid" must not pick up the "validation" shard.
        assert!(select_shards(&listing(), &splits(&["valid"])).is_err());
    }

    #[test]
    fn missing_split_is_an_error() {
        assert!(select_shards(&listing(), &splits(&["dev"])).is_err());
    }

    #[test]
    fn empty_split_list_is_an_error() {
        assert!(select_shards(&listing(), &[]).is_err());
    }

    #[test]
    fn local_shard_names_stay_unique_per_split() {
        assert_eq!(
            local_shard_name("somali/train/0000.parquet"),
            "xlsum_somali_train_0000.parquet"
        );
        assert_ne!(
            local_shard_name("somali/train/0000.parquet"),
            local_shard_name("somali/test/0000.parquet")
        );
    }
}
