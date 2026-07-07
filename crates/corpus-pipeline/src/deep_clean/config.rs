//! v0.2 deep-clean stage configuration. See docs/CLEANING_STRATEGY.md §6.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepCleanConfig {
    #[serde(default = "default_true")]
    pub unescape_madlad: bool,
    #[serde(default = "default_true")]
    pub strip_opus_html: bool,
    #[serde(default = "default_mojibake_passes")]
    pub mojibake_max_passes: usize,
    #[serde(default = "default_true")]
    pub strip_stray_ufffd: bool,
    #[serde(default = "default_true")]
    pub strip_benign_html: bool,
    #[serde(default = "default_true")]
    pub reject_script_html: bool,
    #[serde(default = "default_true")]
    pub mask_urls: bool,
    #[serde(default = "default_true")]
    pub mask_emails: bool,
    #[serde(default = "default_true")]
    pub boilerplate_line_drop: bool,
    #[serde(default = "default_boilerplate_ratio")]
    pub boilerplate_reject_ratio: f64,
    #[serde(default = "default_symbol_reject")]
    pub symbol_ratio_reject: f64,
    #[serde(default = "default_symbol_review")]
    pub symbol_ratio_review: f64,
    #[serde(default = "default_max_words")]
    pub max_document_words: usize,
    #[serde(default)]
    pub intra_dedup: IntraDedupConfig,
    #[serde(default)]
    pub lid: DeepCleanLidConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntraDedupConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_jaccard_tau")]
    pub paragraph_jaccard_tau: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepCleanLidConfig {
    #[serde(default = "default_true")]
    pub segment_level: bool,
    #[serde(default = "default_min_somali_frac")]
    pub min_somali_char_frac: f64,
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f64,
    #[serde(default = "default_clip_bytes")]
    pub clip_bytes: usize,
}

impl Default for DeepCleanConfig {
    fn default() -> Self {
        Self {
            unescape_madlad: true,
            strip_opus_html: true,
            mojibake_max_passes: default_mojibake_passes(),
            strip_stray_ufffd: true,
            strip_benign_html: true,
            reject_script_html: true,
            mask_urls: true,
            mask_emails: true,
            boilerplate_line_drop: true,
            boilerplate_reject_ratio: default_boilerplate_ratio(),
            symbol_ratio_reject: default_symbol_reject(),
            symbol_ratio_review: default_symbol_review(),
            max_document_words: default_max_words(),
            intra_dedup: IntraDedupConfig::default(),
            lid: DeepCleanLidConfig::default(),
        }
    }
}

impl Default for IntraDedupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            paragraph_jaccard_tau: default_jaccard_tau(),
        }
    }
}

impl Default for DeepCleanLidConfig {
    fn default() -> Self {
        Self {
            segment_level: true,
            min_somali_char_frac: default_min_somali_frac(),
            min_confidence: default_min_confidence(),
            clip_bytes: default_clip_bytes(),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_mojibake_passes() -> usize {
    5
}
fn default_boilerplate_ratio() -> f64 {
    0.40
}
fn default_symbol_reject() -> f64 {
    0.45
}
fn default_symbol_review() -> f64 {
    0.35
}
fn default_max_words() -> usize {
    10_000
}
fn default_jaccard_tau() -> f64 {
    0.95
}
fn default_min_somali_frac() -> f64 {
    0.60
}
fn default_min_confidence() -> f64 {
    0.50
}
fn default_clip_bytes() -> usize {
    1024
}
