use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Canonical schema version. Increment on breaking record changes.
pub const SCHEMA_VERSION: u16 = 1;

// ── Newtypes ──────────────────────────────────────────────────────────────────

/// Stable document identifier (see docs/ID_STRATEGY.md).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocId(pub String);

/// Registry key for a data source (e.g. `"hplt"`, `"wikipedia"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceKey(pub String);

/// SHA-256 hex digest used for exact dedup (see docs/ID_STRATEGY.md).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash(pub String);

/// BCP-47 language tag (e.g. `"so"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Lang(pub String);

// ── Licensing ─────────────────────────────────────────────────────────────────

/// Per-source license identifier. There is no single corpus-wide license.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum License {
    #[serde(rename = "CC0-1.0")]
    Cc0_1_0,
    #[serde(rename = "CC-BY-4.0")]
    CcBy4_0,
    #[serde(rename = "CC-BY-SA-4.0")]
    CcBySa4_0,
    #[serde(rename = "MIT")]
    Mit,
    #[serde(rename = "Apache-2.0")]
    Apache2_0,
    #[serde(rename = "public-domain")]
    PublicDomain,
    #[serde(untagged)]
    Other(String),
}

// ── Raw stage (download + merge today) ───────────────────────────────────────

/// Minimal record produced by downloaders and merge today.
///
/// See docs/METADATA_SCHEMA.md § Raw records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRecord {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Backward-compatible alias for [`RawRecord`].
pub type Document = RawRecord;

// ── Provenance ────────────────────────────────────────────────────────────────

/// Where a record came from and when it was captured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub source: SourceKey,
    pub collected_at: DateTime<Utc>,
    pub lang: Lang,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subsource: Option<String>,
}

// ── Quality & dedup metadata (populated by future pipeline stages) ────────────

/// Why a record failed a quality gate. Multiple flags may apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityFlag {
    TooShort,
    TooLong,
    HighSymbolRatio,
    LowLangScore,
    HtmlRemnant,
    MostlyNumbers,
    RepeatedNgrams,
    NotSomali,
    /// Excessive U+FFFD replacement characters (heavily corrupted encoding).
    Corrupted,
}

/// Whether a record is kept, rejected, or held for review.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordDisposition {
    #[default]
    Kept,
    Rejected,
    Review,
}

/// Near- and exact-duplicate metadata (see docs/QUALITY_METADATA.md).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DedupInfo {
    pub is_duplicate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_of: Option<DocId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_duplicate_of: Option<DocId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<ContentHash>,
}

/// Quality gate outcomes. Failed records are preserved with `disposition != kept`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityInfo {
    pub disposition: RecordDisposition,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<QualityFlag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_ratio: Option<f32>,
}

// ── Corpus record (target processed shape) ───────────────────────────────────

/// Full corpus record used from cleaning onward.
///
/// See docs/METADATA_SCHEMA.md § Corpus records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusRecord {
    pub id: DocId,
    pub text: String,
    pub provenance: Provenance,
    pub license: License,
    pub content_hash: ContentHash,
    pub dedup: DedupInfo,
    pub quality: QualityInfo,
    pub schema_version: u16,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, serde_json::Value>,
}
