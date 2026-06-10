//! Language identification backends behind a common interface, so the benchmark
//! and the LID stage share one detection path. See docs/CLEANING_PLAN.md §3.

use crate::config::LidBackend;

pub mod stage;

/// A language detector returning a normalized top-1 prediction and confidence.
pub trait Detector {
    fn name(&self) -> &'static str;

    /// Detect the top language. Returns `(code, confidence)` where `code` is an
    /// ISO code (`"so"` for Somali) and confidence is in `[0, 1]`.
    fn detect(&self, text: &str) -> Option<(String, f64)>;

    /// Convenience: is the top-1 prediction Somali with confidence >= `min`?
    fn is_somali(&self, text: &str, min_confidence: f64) -> bool {
        match self.detect(text) {
            Some((code, conf)) => code == "so" && conf >= min_confidence,
            None => false,
        }
    }
}

/// Build a detector for the given backend.
pub fn build(backend: LidBackend) -> Box<dyn Detector> {
    match backend {
        LidBackend::Whatlang => Box::new(WhatlangDetector),
        LidBackend::Lingua => Box::new(LinguaDetector::new()),
    }
}

/// whatlang backend. Lightweight, trigram-based.
pub struct WhatlangDetector;

impl Detector for WhatlangDetector {
    fn name(&self) -> &'static str {
        "whatlang"
    }

    fn detect(&self, text: &str) -> Option<(String, f64)> {
        let info = whatlang::detect(text)?;
        // whatlang reports ISO 639-3; normalize Somali to "so".
        let raw = info.lang().code();
        let code = if raw == "som" { "so".to_string() } else { raw.to_string() };
        Some((code, info.confidence()))
    }
}

/// lingua backend. Higher claimed accuracy, especially on short text.
pub struct LinguaDetector {
    detector: lingua::LanguageDetector,
}

impl LinguaDetector {
    pub fn new() -> Self {
        use lingua::Language::{
            Arabic, English, French, Indonesian, Italian, Portuguese, Somali, Spanish, Swahili,
            Tagalog, Turkish,
        };
        // Somali plus confusable / common Latin-script languages it supports.
        let languages = vec![
            Somali, Swahili, English, Italian, Arabic, French, Spanish, Portuguese, Turkish,
            Indonesian, Tagalog,
        ];
        let detector = lingua::LanguageDetectorBuilder::from_languages(&languages).build();
        Self { detector }
    }
}

impl Default for LinguaDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for LinguaDetector {
    fn name(&self) -> &'static str {
        "lingua"
    }

    fn detect(&self, text: &str) -> Option<(String, f64)> {
        let predicted = self.detector.detect_language_of(text)?;
        let confidence = self.detector.compute_language_confidence(text, predicted);
        let code = if predicted == lingua::Language::Somali {
            "so".to_string()
        } else {
            predicted.iso_code_639_1().to_string().to_lowercase()
        };
        Some((code, confidence))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whatlang_detects_somali_text() {
        let d = WhatlangDetector;
        let result = d.detect(
            "Soomaaliya waa dal ku yaal Geeska Afrika, waxayna leedahay xeeb dheer.",
        );
        assert!(result.is_some());
    }

    #[test]
    fn lingua_detects_somali_over_english() {
        let d = LinguaDetector::new();
        let (code, conf) = d
            .detect("Soomaaliya waa dal ku yaal Geeska Afrika oo leh dhaqan hodan ah.")
            .unwrap();
        assert_eq!(code, "so", "expected Somali, got {code} ({conf})");
    }
}
