use std::fs::File;
use std::io::{copy, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use serde::Deserialize;

/// Default branch: every dataset that ships its files on `main`.
pub const MAIN_REVISION: &str = "main";

/// Auto-converted parquet branch the Hub publishes for script-based datasets
/// (those the modern `datasets` library can no longer load by config name).
pub const PARQUET_REVISION: &str = "refs/convert/parquet";

/// Percent-encode a git revision so refs like `refs/convert/parquet` survive as
/// a single URL path segment — the Hub returns 404 for unencoded slashes.
fn encode_revision(revision: &str) -> String {
    revision.replace('/', "%2F")
}

#[derive(Debug, Deserialize)]
struct TreeEntry {
    #[serde(rename = "type")]
    entry_type: String,
    path: String,
}

pub struct HfClient {
    client: Client,
    token: Option<String>,
}

impl Default for HfClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HfClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("corpus-tools/0.1")
                .build()
                .expect("reqwest client"),
            token: resolve_token(),
        }
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn list_files(&self, repo: &str, prefix: &str) -> Result<Vec<String>> {
        self.list_files_at(repo, MAIN_REVISION, prefix, false)
    }

    pub fn list_files_recursive(&self, repo: &str, prefix: &str) -> Result<Vec<String>> {
        self.list_files_at(repo, MAIN_REVISION, prefix, true)
    }

    /// List files under `prefix` at an arbitrary revision (branch, tag, or ref
    /// such as [`PARQUET_REVISION`]).
    pub fn list_files_at(
        &self,
        repo: &str,
        revision: &str,
        prefix: &str,
        recursive: bool,
    ) -> Result<Vec<String>> {
        let revision = encode_revision(revision);
        let query = if recursive { "?recursive=true" } else { "" };
        let url =
            format!("https://huggingface.co/api/datasets/{repo}/tree/{revision}/{prefix}{query}");
        let mut request = self.client.get(&url);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }

        let response = request
            .send()
            .context("listing Hugging Face dataset files")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            bail!("dataset path not found: {repo}@{revision}/{prefix}");
        }
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            bail!("authentication required for {repo}");
        }
        if !response.status().is_success() {
            bail!(
                "failed to list files for {repo}@{revision}/{prefix}: {}",
                response.status()
            );
        }

        let entries: Vec<TreeEntry> = response.json().context("parsing HF tree response")?;
        Ok(entries
            .into_iter()
            .filter(|entry| entry.entry_type == "file")
            .map(|entry| entry.path)
            .collect())
    }

    pub fn download_to_path(
        &self,
        repo: &str,
        remote_path: &str,
        destination: &Path,
    ) -> Result<()> {
        self.download_to_path_at(repo, MAIN_REVISION, remote_path, destination)
    }

    /// Download a single file from an arbitrary revision.
    pub fn download_to_path_at(
        &self,
        repo: &str,
        revision: &str,
        remote_path: &str,
        destination: &Path,
    ) -> Result<()> {
        let revision = encode_revision(revision);
        let url =
            format!("https://huggingface.co/datasets/{repo}/resolve/{revision}/{remote_path}");
        let mut request = self.client.get(&url);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }

        let mut response = request.send().context("downloading Hugging Face file")?;
        if !response.status().is_success() {
            bail!(
                "failed to download {repo}/{remote_path}: {}",
                response.status()
            );
        }

        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let total = response.content_length();
        let progress = ProgressBar::new(total.unwrap_or(0));
        progress.set_style(
            ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes}")
                .context("progress template")?
                .progress_chars("=>-"),
        );
        progress.set_message(format!("Downloading {remote_path}"));

        let mut file = File::create(destination)?;
        if let Some(total) = total {
            let mut downloaded = 0u64;
            let mut buffer = vec![0u8; 1024 * 1024];
            while downloaded < total {
                let read = response.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                file.write_all(&buffer[..read])?;
                downloaded += read as u64;
                progress.set_position(downloaded);
            }
        } else {
            copy(&mut response, &mut file)?;
        }
        progress.finish_and_clear();
        Ok(())
    }

    pub fn download_to_temp(
        &self,
        repo: &str,
        remote_path: &str,
    ) -> Result<(tempfile::NamedTempFile, PathBuf)> {
        self.download_to_temp_at(repo, MAIN_REVISION, remote_path)
    }

    /// Download a single file from an arbitrary revision into a temp file whose
    /// lifetime the caller owns.
    pub fn download_to_temp_at(
        &self,
        repo: &str,
        revision: &str,
        remote_path: &str,
    ) -> Result<(tempfile::NamedTempFile, PathBuf)> {
        let suffix = Path::new(remote_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| format!(".{ext}"))
            .unwrap_or_default();
        let temp = tempfile::Builder::new().suffix(&suffix).tempfile()?;
        let path = temp.path().to_path_buf();
        self.download_to_path_at(repo, revision, remote_path, &path)?;
        Ok((temp, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_slashes_in_refs() {
        assert_eq!(
            encode_revision("refs/convert/parquet"),
            "refs%2Fconvert%2Fparquet"
        );
    }

    #[test]
    fn leaves_plain_branch_untouched() {
        assert_eq!(encode_revision("main"), "main");
    }
}

pub fn resolve_token() -> Option<String> {
    std::env::var("HF_TOKEN")
        .ok()
        .or_else(|| std::env::var("HUGGING_FACE_HUB_TOKEN").ok())
        .filter(|token| !token.trim().is_empty())
}

pub fn filter_paths(paths: Vec<String>, suffix: &str) -> Vec<String> {
    let mut filtered: Vec<String> = paths
        .into_iter()
        .filter(|path| path.ends_with(suffix))
        .collect();
    filtered.sort();
    filtered
}
