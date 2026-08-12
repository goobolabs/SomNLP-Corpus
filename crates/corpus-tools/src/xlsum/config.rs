//! Source configuration and the on-disk layout every XL-Sum stage writes to.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::hf::PARQUET_REVISION;

pub const SOURCE_TAG: &str = "xlsum";
pub const DATASET_NAME: &str = "csebuetnlp/xlsum";
pub const DATASET_CONFIG: &str = "somali";

/// Splits published for the Somali configuration.
pub const SPLITS: [&str; 3] = ["train", "validation", "test"];

/// Parquet columns that hold text worth exporting as a record's `text`.
pub const FIELDS: [&str; 3] = ["text", "summary", "title"];

/// Every column the pipeline reads: the three text fields plus provenance.
pub const COLUMNS: [&str; 5] = ["id", "url", "title", "text", "summary"];

/// Default root for the staged pipeline artefacts.
pub const DEFAULT_ROOT: &str = "data/raw/xlsum";

/// Human-readable provenance line for the export summary.
pub fn source_url() -> String {
    format!(
        "https://huggingface.co/datasets/{DATASET_NAME} (config: {DATASET_CONFIG}, \
         revision: {PARQUET_REVISION})"
    )
}

/// All splits, as owned strings — the default for every stage.
pub fn default_splits() -> Vec<String> {
    SPLITS.iter().map(|split| split.to_string()).collect()
}

/// The artefact tree the staged pipeline produces under a single root.
///
/// Mirrors the layout of the reference Python toolkit so both produce
/// interchangeable outputs:
///
/// ```text
/// <root>/
/// ├── raw/                          downloaded parquet shards
/// ├── extracted.jsonl               extracted (then cleaned) records
/// ├── corpus.jsonl / .csv / .txt    exported corpus
/// └── reports/schema.json, metadata_report.json
/// ```
#[derive(Debug, Clone)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn raw_dir(&self) -> PathBuf {
        self.root.join("raw")
    }

    pub fn reports_dir(&self) -> PathBuf {
        self.root.join("reports")
    }

    pub fn extracted(&self) -> PathBuf {
        self.root.join("extracted.jsonl")
    }

    pub fn corpus_jsonl(&self) -> PathBuf {
        self.root.join("corpus.jsonl")
    }

    pub fn corpus_csv(&self) -> PathBuf {
        self.root.join("corpus.csv")
    }

    pub fn corpus_txt(&self) -> PathBuf {
        self.root.join("corpus.txt")
    }

    pub fn schema_report(&self) -> PathBuf {
        self.reports_dir().join("schema.json")
    }

    pub fn stats_report(&self) -> PathBuf {
        self.reports_dir().join("metadata_report.json")
    }

    /// Create every directory the pipeline writes to.
    pub fn ensure_dirs(&self) -> Result<()> {
        for directory in [self.root.clone(), self.raw_dir(), self.reports_dir()] {
            fs::create_dir_all(&directory)
                .with_context(|| format!("creating directory {}", directory.display()))?;
        }
        Ok(())
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new(DEFAULT_ROOT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_paths_hang_off_the_root() {
        let layout = Layout::new("out/xlsum");
        assert_eq!(layout.raw_dir(), Path::new("out/xlsum/raw"));
        assert_eq!(layout.extracted(), Path::new("out/xlsum/extracted.jsonl"));
        assert_eq!(
            layout.stats_report(),
            Path::new("out/xlsum/reports/metadata_report.json")
        );
    }

    #[test]
    fn source_url_names_the_parquet_revision() {
        assert!(source_url().contains(PARQUET_REVISION));
    }
}
