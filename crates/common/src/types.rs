use serde::{Deserialize, Serialize};

/// A single corpus document as produced by the dataset downloaders.
///
/// At this early stage a record is just the text plus an optional source tag
/// identifying which dataset it came from. Most sources emit text only; a few
/// (such as parallel corpora) also record a source. The schema grows as later
/// pipeline stages — cleaning, language identification, deduplication — are added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    /// The document text.
    pub text: String,

    /// Identifier of the dataset this document came from (e.g. `"mt560"`),
    /// when the source records one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}
