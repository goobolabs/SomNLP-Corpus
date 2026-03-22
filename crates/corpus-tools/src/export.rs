use std::path::{Path, PathBuf};

use anyhow::Result;
use tempfile::NamedTempFile;

use crate::hf::HfClient;
use crate::jsonl::{is_non_empty, JsonlWriter};

fn resolve_local_shard(
    hf: &HfClient,
    repo: &str,
    remote_path: &str,
    output: &Path,
    streaming: bool,
    temps: &mut Vec<NamedTempFile>,
) -> Result<PathBuf> {
    if streaming {
        let (temp, path) = hf.download_to_temp(repo, remote_path)?;
        temps.push(temp);
        Ok(path)
    } else {
        let path = output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(Path::new(remote_path).file_name().unwrap_or_default());
        hf.download_to_path(repo, remote_path, &path)?;
        Ok(path)
    }
}

pub fn export_parquet_shards(
    hf: &HfClient,
    repo: &str,
    shard_paths: &[String],
    output: &Path,
    column: &str,
    limit: Option<u64>,
    streaming: bool,
    title: &str,
    source: &str,
    record_source: Option<&str>,
) -> Result<crate::Stats> {
    if shard_paths.is_empty() {
        anyhow::bail!("no dataset shards found to download");
    }

    let mut writer = JsonlWriter::create(output, "Writing")?;
    let mut written = 0u64;
    let mut temps = Vec::new();

    for remote_path in shard_paths {
        if limit.is_some_and(|limit| written >= limit) {
            break;
        }

        let local_path =
            resolve_local_shard(hf, repo, remote_path, output, streaming, &mut temps)?;

        for text in crate::parquet_source::iter_text_column(&local_path, column)? {
            if limit.is_some_and(|limit| written >= limit) {
                break;
            }
            let text = text?;
            if !is_non_empty(&text) {
                continue;
            }
            if let Some(tag) = record_source {
                writer.write_tagged(&text, tag)?;
            } else {
                writer.write_text(&text)?;
            }
            written += 1;
        }
    }

    let stats = writer.stats.clone();
    writer.finish();

    if stats.total_docs == 0 {
        anyhow::bail!("no documents exported from {repo}");
    }

    crate::cli::print_export_summary(title, &stats, output, source);
    Ok(stats)
}

pub fn export_parquet_struct_shards(
    hf: &HfClient,
    repo: &str,
    shard_paths: &[String],
    output: &Path,
    column: &str,
    field: &str,
    limit: Option<u64>,
    streaming: bool,
    title: &str,
    source: &str,
) -> Result<crate::Stats> {
    if shard_paths.is_empty() {
        anyhow::bail!("no dataset shards found to download");
    }

    let mut writer = JsonlWriter::create(output, "Writing")?;
    let mut written = 0u64;
    let mut temps = Vec::new();

    for remote_path in shard_paths {
        if limit.is_some_and(|limit| written >= limit) {
            break;
        }

        let local_path =
            resolve_local_shard(hf, repo, remote_path, output, streaming, &mut temps)?;

        for text in crate::parquet_source::iter_struct_field(&local_path, column, field)? {
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
        anyhow::bail!("no documents exported from {repo}");
    }

    crate::cli::print_export_summary(title, &stats, output, source);
    Ok(stats)
}

pub fn export_json_gz_shards(
    hf: &HfClient,
    repo: &str,
    shard_paths: &[String],
    output: &Path,
    limit: Option<u64>,
    streaming: bool,
    title: &str,
    source: &str,
) -> Result<crate::Stats> {
    if shard_paths.is_empty() {
        anyhow::bail!("no dataset shards found to download");
    }

    let mut writer = JsonlWriter::create(output, "Writing")?;
    let mut written = 0u64;
    let mut temps = Vec::new();

    for remote_path in shard_paths {
        if limit.is_some_and(|limit| written >= limit) {
            break;
        }

        let local_path =
            resolve_local_shard(hf, repo, remote_path, output, streaming, &mut temps)?;

        for text in crate::parquet_source::iter_json_gz_text(&local_path)? {
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
        anyhow::bail!("no documents exported from {repo}");
    }

    crate::cli::print_export_summary(title, &stats, output, source);
    Ok(stats)
}
