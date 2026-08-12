//! The record shape carried between the extract, clean and export stages.

use serde::{Deserialize, Serialize};

/// One XL-Sum article.
///
/// `id` / `url` / `split` are kept purely for traceability; the corpus signal
/// lives in `title` / `text` / `summary`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct XlsumRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub split: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub summary: String,
}

impl XlsumRecord {
    /// Read the field named by [`crate::xlsum::config::FIELDS`].
    pub fn field(&self, field: &str) -> Option<&str> {
        match field {
            "text" => Some(&self.text),
            "summary" => Some(&self.summary),
            "title" => Some(&self.title),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_field_is_none() {
        let record = XlsumRecord::default();
        assert!(record.field("id").is_none());
        assert!(record.field("text").is_some());
    }

    #[test]
    fn missing_json_keys_default_to_empty() {
        let record: XlsumRecord = serde_json::from_str(r#"{"text":"waa dal"}"#).unwrap();
        assert_eq!(record.text, "waa dal");
        assert!(record.url.is_empty());
    }
}
