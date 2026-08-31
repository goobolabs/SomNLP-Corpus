//! Per-source raw file paths under `data/raw/`. Most sources use
//! `{source}_so.jsonl`; Qur'an downloaders use `translation.json` (+ footnotes).

use std::path::{Path, PathBuf};

/// All JSONL inputs for one registry source key, in read order.
pub fn source_raw_paths(raw_dir: &Path, source: &str) -> Vec<PathBuf> {
    let dir = raw_dir.join(source);
    match source {
        "quran" => vec![dir.join("translation.json"), dir.join("footnotes.json")],
        "quran-tanzil" => vec![dir.join("translation.json")],
        _ => vec![dir.join(format!("{source}_so.jsonl"))],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_source_uses_so_jsonl() {
        let paths = source_raw_paths(Path::new("data/raw"), "hplt");
        assert_eq!(paths, vec![PathBuf::from("data/raw/hplt/hplt_so.jsonl")]);
    }

    #[test]
    fn quran_uses_translation_and_footnotes() {
        let paths = source_raw_paths(Path::new("data/raw"), "quran");
        assert_eq!(
            paths,
            vec![
                PathBuf::from("data/raw/quran/translation.json"),
                PathBuf::from("data/raw/quran/footnotes.json"),
            ]
        );
    }

    #[test]
    fn tanzil_uses_translation_only() {
        let paths = source_raw_paths(Path::new("data/raw"), "quran-tanzil");
        assert_eq!(
            paths,
            vec![PathBuf::from("data/raw/quran-tanzil/translation.json")]
        );
    }
}
