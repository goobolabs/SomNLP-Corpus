//! Pipeline configuration loaded from `configs/pipeline.toml`. One versioned
//! file holds every knob for reproducibility (see docs/CLEANING_PLAN.md).

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub version: String,
    pub merge_source_order: Vec<String>,
    pub clean: CleanConfig,
    pub lid: LidConfig,
    pub near_dedup: NearDedupConfig,
    #[serde(default)]
    pub deep_clean: crate::deep_clean::DeepCleanConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanConfig {
    pub document_min_words: usize,
    pub sentence_min_words: usize,
    pub ufffd_reject_ratio: f64,
    pub max_repeated_run: usize,
    pub symbol_ratio_review: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidConfig {
    pub backend: LidBackend,
    pub min_confidence: f64,
    pub detect_clip_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LidBackend {
    Whatlang,
    Lingua,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearDedupConfig {
    pub shingle_k: usize,
    pub k_hashes: usize,
    pub bands: usize,
    pub rows: usize,
    pub tau: f64,
    pub seed: u64,
}

impl PipelineConfig {
    /// Load and validate the config from a TOML file.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        let config: PipelineConfig =
            toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.near_dedup.bands * self.near_dedup.rows == self.near_dedup.k_hashes,
            "near_dedup: bands * rows ({} * {}) must equal k_hashes ({})",
            self.near_dedup.bands,
            self.near_dedup.rows,
            self.near_dedup.k_hashes
        );
        anyhow::ensure!(
            (0.0..=1.0).contains(&self.near_dedup.tau),
            "near_dedup.tau must be in [0, 1]"
        );
        anyhow::ensure!(
            !self.merge_source_order.is_empty(),
            "merge_source_order must not be empty"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
version = "1"
merge_source_order = ["mt560", "opus", "cc100", "mc4", "madlad", "hplt"]

[clean]
document_min_words = 25
sentence_min_words = 5
ufffd_reject_ratio = 0.005
max_repeated_run = 3
symbol_ratio_review = 0.5

[lid]
backend = "lingua"
min_confidence = 0.50
detect_clip_bytes = 2000

[near_dedup]
shingle_k = 3
k_hashes = 64
bands = 16
rows = 4
tau = 0.80
seed = 0
"#;

    #[test]
    fn parses_sample_config() {
        let config: PipelineConfig = toml::from_str(SAMPLE).unwrap();
        config.validate().unwrap();
        assert_eq!(config.merge_source_order.len(), 6);
        assert_eq!(config.lid.backend, LidBackend::Lingua);
        assert_eq!(config.near_dedup.k_hashes, 64);
    }

    #[test]
    fn rejects_inconsistent_lsh_shape() {
        let mut config: PipelineConfig = toml::from_str(SAMPLE).unwrap();
        config.near_dedup.bands = 8;
        assert!(config.validate().is_err());
    }

    #[test]
    fn roundtrips_through_toml() {
        let config: PipelineConfig = toml::from_str(SAMPLE).unwrap();
        let encoded = toml::to_string(&config).unwrap();
        let decoded: PipelineConfig = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded.merge_source_order, config.merge_source_order);
    }
}
