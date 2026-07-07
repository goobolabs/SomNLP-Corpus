//! Segment-level LID for deep clean (document class only).

use common::registry::{self, LidPolicy, SourceClass};
use common::types::QualityFlag;

use crate::deep_clean::DeepCleanLidConfig;
use crate::lid::Detector;

const SOMALI: &str = "so";

fn clip(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn is_somali_segment(detector: &dyn Detector, text: &str, min_confidence: f64, clip_bytes: usize) -> bool {
    let snippet = clip(text, clip_bytes);
    if snippet.trim().is_empty() {
        return true;
    }
    match detector.detect(snippet) {
        Some((code, conf)) => code == SOMALI && conf >= min_confidence,
        None => false,
    }
}

pub fn check_segment_lid(
    source: &str,
    text: &str,
    cfg: &DeepCleanLidConfig,
    detector: &dyn Detector,
) -> Result<(), QualityFlag> {
    if !cfg.segment_level {
        return Ok(());
    }
    let policy = registry::lookup(source)
        .map(|entry| entry.lid_policy)
        .unwrap_or(LidPolicy::Gate);
    if policy != LidPolicy::Gate {
        return Ok(());
    }
    let class = registry::lookup(source)
        .map(|entry| entry.class)
        .unwrap_or(SourceClass::Document);
    if class != SourceClass::Document {
        return Ok(());
    }

    let segments: Vec<&str> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if segments.is_empty() {
        return Ok(());
    }

    let mut somali_chars = 0usize;
    let mut total_chars = 0usize;
    for segment in segments {
        let len = segment.chars().count();
        total_chars += len;
        if is_somali_segment(detector, segment, cfg.min_confidence, cfg.clip_bytes) {
            somali_chars += len;
        }
    }
    let frac = if total_chars == 0 {
        1.0
    } else {
        somali_chars as f64 / total_chars as f64
    };
    if frac < cfg.min_somali_char_frac {
        return Err(QualityFlag::NotSomali);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lid::LinguaDetector;

    #[test]
    fn keeps_mostly_somali_document() {
        let d = LinguaDetector::new();
        let cfg = DeepCleanLidConfig::default();
        let text = "Soomaaliya waa dal ku yaal Geeska Afrika.\n\nWaxay leedahay taariikh dheer.";
        assert!(check_segment_lid("hplt", text, &cfg, &d).is_ok());
    }
}
