//! Source constants and the on-disk layout every NLLB stage writes to.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use reqwest::Url;

pub const SOURCE_TAG: &str = "nllb";
pub const DATASET_REPO: &str = "allenai/nllb";
pub const LANGUAGE_PAIR: &str = "eng_Latn-som_Latn";
pub const ENGLISH_CODE: &str = "eng_Latn";
pub const SOMALI_CODE: &str = "som_Latn";

/// Bucket the official `allenai/nllb` loader resolves language pairs against:
///
/// ```text
/// _ALLENAI_URL = "https://storage.googleapis.com/allennlp-data-bucket/nllb/"
/// url = f"{_ALLENAI_URL}{src_lg}-{tgt_lg}.gz"
/// ```
///
/// Only the single English–Somali pair file is ever fetched, never the whole
/// NLLB corpus, and the deprecated dataset script is not involved.
pub const ALLENAI_URL: &str = "https://storage.googleapis.com/allennlp-data-bucket/nllb/";

/// Hosts a download URL — or any redirect hop — is allowed to stay on.
pub const OFFICIAL_HOSTS: [&str; 2] = [
    "storage.googleapis.com",
    "allennlp-data-bucket.storage.googleapis.com",
];

/// The nine raw TSV fields, in file order. This order is never changed
/// silently: it is the CSV/TSV column order and the JSONL key order.
pub const FIELD_NAMES: [&str; 9] = [
    "english",
    "somali",
    "laser_score",
    "english_lid_score",
    "somali_lid_score",
    "english_source",
    "english_url",
    "somali_source",
    "somali_url",
];

pub const NUM_FIELDS: usize = FIELD_NAMES.len();

/// URL placeholder upstream writes for "no URL"; decoded as `null`.
pub const MISSING_URL_TOKEN: &str = "_";

/// The raw archive's name on disk (`{LANGUAGE_PAIR}.gz`).
pub const RAW_FILENAME: &str = "eng_Latn-som_Latn.gz";

pub const OUT_JSONL: &str = "nllb_eng_som.jsonl";
pub const OUT_CSV: &str = "nllb_eng_som.csv";
pub const OUT_TSV: &str = "nllb_eng_som.tsv";
pub const OUT_SOMALI: &str = "nllb_somali.txt";
pub const OUT_PARALLEL_EN: &str = "nllb_parallel.en";
pub const OUT_PARALLEL_SO: &str = "nllb_parallel.so";
pub const OUT_AUDIT_JSON: &str = "audit_report.json";
pub const OUT_AUDIT_MD: &str = "audit_report.md";
pub const OUT_SAMPLES: &str = "sample_pairs.jsonl";

/// Default output root, matching the `data/raw/<source>` layout of every other
/// downloader in this crate.
pub const DEFAULT_ROOT: &str = "data/raw/nllb";

/// Caveats copied verbatim into every audit report — NLLB is mined data and
/// must not be presented as verified translation.
pub const AUDIT_WARNINGS: [&str; 6] = [
    "NLLB is machine-mined parallel data, not fully human-verified translation data.",
    "LASER and language-ID scores are useful signals but do not guarantee correctness.",
    "Low-scoring rows may contain alignment errors.",
    "Source text may include boilerplate, machine translation, toxic content, sensitive \
     information, or personal information.",
    "NLLB should not be used as an evaluation benchmark.",
    "Extracted Somali text requires further cleaning before language-model training.",
];

/// The official English–Somali archive URL.
pub fn official_url() -> String {
    format!("{ALLENAI_URL}{LANGUAGE_PAIR}.gz")
}

/// Human-readable provenance line for the run summary.
pub fn source_url() -> String {
    format!("{DATASET_REPO} ({LANGUAGE_PAIR}) via {}", official_url())
}

/// Whether `url` points at the official AllenAI bucket.
///
/// Applied to the start URL *and* to every redirect hop, so a hijacked
/// redirect cannot silently swap in a mirror.
pub fn is_official_host(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    OFFICIAL_HOSTS.contains(&host.as_str()) || host.ends_with(".storage.googleapis.com")
}

/// Every file the tool writes, hanging off one output root:
///
/// ```text
/// <root>/
/// ├── eng_Latn-som_Latn.gz                   raw archive (preserved)
/// ├── nllb_eng_som.jsonl / .csv / .tsv       converted corpus
/// ├── nllb_somali.txt                        Somali-only text
/// ├── nllb_parallel.en / .so                 line-synchronized pair files
/// └── audit_report.json / .md, sample_pairs.jsonl
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

    pub fn raw_archive(&self) -> PathBuf {
        self.root.join(RAW_FILENAME)
    }

    pub fn jsonl(&self) -> PathBuf {
        self.root.join(OUT_JSONL)
    }

    pub fn csv(&self) -> PathBuf {
        self.root.join(OUT_CSV)
    }

    pub fn tsv(&self) -> PathBuf {
        self.root.join(OUT_TSV)
    }

    pub fn somali_txt(&self) -> PathBuf {
        self.root.join(OUT_SOMALI)
    }

    pub fn parallel_en(&self) -> PathBuf {
        self.root.join(OUT_PARALLEL_EN)
    }

    pub fn parallel_so(&self) -> PathBuf {
        self.root.join(OUT_PARALLEL_SO)
    }

    pub fn audit_json(&self) -> PathBuf {
        self.root.join(OUT_AUDIT_JSON)
    }

    pub fn audit_md(&self) -> PathBuf {
        self.root.join(OUT_AUDIT_MD)
    }

    pub fn samples(&self) -> PathBuf {
        self.root.join(OUT_SAMPLES)
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("creating directory {}", self.root.display()))
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
    fn raw_filename_matches_the_language_pair() {
        assert_eq!(RAW_FILENAME, format!("{LANGUAGE_PAIR}.gz"));
    }

    #[test]
    fn official_url_is_the_single_pair_file() {
        assert_eq!(
            official_url(),
            "https://storage.googleapis.com/allennlp-data-bucket/nllb/eng_Latn-som_Latn.gz"
        );
    }

    #[test]
    fn official_bucket_and_its_subdomains_are_accepted() {
        assert!(is_official_host(&official_url()));
        assert!(is_official_host(
            "https://allennlp-data-bucket.storage.googleapis.com/nllb/x.gz"
        ));
    }

    #[test]
    fn other_hosts_are_rejected() {
        assert!(!is_official_host("https://evil.example/x.gz"));
        // A look-alike suffix must not pass: the check is on the host label.
        assert!(!is_official_host("https://storage.googleapis.com.evil.test/x.gz"));
        assert!(!is_official_host("not a url"));
    }

    #[test]
    fn layout_paths_hang_off_the_root() {
        let layout = Layout::new("out/nllb");
        assert_eq!(layout.raw_archive(), Path::new("out/nllb/eng_Latn-som_Latn.gz"));
        assert_eq!(layout.jsonl(), Path::new("out/nllb/nllb_eng_som.jsonl"));
        assert_eq!(layout.audit_md(), Path::new("out/nllb/audit_report.md"));
    }
}
