//! Raw TSV parsing and the record shape written to every output format.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::nllb::config::{LANGUAGE_PAIR, MISSING_URL_TOKEN, NUM_FIELDS};

/// One parsed raw row, with the nine fields kept in file order.
#[derive(Debug, Clone, PartialEq)]
pub struct RawRow {
    pub english: String,
    pub somali: String,
    pub laser_score: f64,
    pub english_lid_score: f64,
    pub somali_lid_score: f64,
    pub english_source: String,
    pub english_url: Option<String>,
    pub somali_source: String,
    pub somali_url: Option<String>,
}

/// Why a raw line could not become a [`RawRow`].
///
/// Both variants are counted in the audit report rather than aborting the run:
/// a handful of broken lines must not cost a multi-gigabyte download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowError {
    /// The line did not have exactly [`NUM_FIELDS`] tab-separated fields.
    Malformed { found: usize },
    /// A LASER/LID field could not be parsed as a float.
    InvalidNumeric { field: &'static str },
}

impl fmt::Display for RowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RowError::Malformed { found } => {
                write!(formatter, "expected {NUM_FIELDS} fields, got {found}")
            }
            RowError::InvalidNumeric { field } => {
                write!(formatter, "invalid numeric value in field '{field}'")
            }
        }
    }
}

impl std::error::Error for RowError {}

/// Parse one raw TSV line.
///
/// The `_` URL placeholder becomes `None`; field order is preserved exactly.
pub fn parse_raw_line(line: &str) -> Result<RawRow, RowError> {
    let stripped = line.trim_end_matches(['\r', '\n']);
    let fields: Vec<&str> = stripped.split('\t').collect();
    if fields.len() != NUM_FIELDS {
        return Err(RowError::Malformed {
            found: fields.len(),
        });
    }

    Ok(RawRow {
        english: fields[0].to_string(),
        somali: fields[1].to_string(),
        laser_score: parse_score(fields[2], "laser_score")?,
        english_lid_score: parse_score(fields[3], "english_lid_score")?,
        somali_lid_score: parse_score(fields[4], "somali_lid_score")?,
        english_source: fields[5].to_string(),
        english_url: clean_url(fields[6]),
        somali_source: fields[7].to_string(),
        somali_url: clean_url(fields[8]),
    })
}

fn parse_score(value: &str, field: &'static str) -> Result<f64, RowError> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|_| RowError::InvalidNumeric { field })
}

fn clean_url(value: &str) -> Option<String> {
    let value = value.trim();
    if value == MISSING_URL_TOKEN || value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Character-length ratio of a pair, always `>= 1.0`.
pub fn length_ratio(english: &str, somali: &str) -> f64 {
    let english = english.chars().count();
    let somali = somali.chars().count();
    english.max(somali) as f64 / english.min(somali).max(1) as f64
}

/// The canonical exported record: a raw row plus its stable id and the
/// language pair it came from. Missing URLs stay `null` — nothing is invented.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NllbRecord {
    pub id: String,
    pub english: String,
    pub somali: String,
    pub laser_score: f64,
    pub english_lid_score: f64,
    pub somali_lid_score: f64,
    pub english_source: String,
    pub english_url: Option<String>,
    pub somali_source: String,
    pub somali_url: Option<String>,
    pub language_pair: String,
}

/// Build the exported record for the `index`-th accepted pair (1-based).
pub fn build_record(row: &RawRow, index: u64) -> NllbRecord {
    NllbRecord {
        id: format!("nllb-eng-som-{index:08}"),
        english: row.english.clone(),
        somali: row.somali.clone(),
        laser_score: row.laser_score,
        english_lid_score: row.english_lid_score,
        somali_lid_score: row.somali_lid_score,
        english_source: row.english_source.clone(),
        english_url: row.english_url.clone(),
        somali_source: row.somali_source.clone(),
        somali_url: row.somali_url.clone(),
        language_pair: LANGUAGE_PAIR.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(fields: [&str; 9]) -> String {
        fields.join("\t")
    }

    fn sample() -> String {
        row([
            "hello", "salaan", "1.2000", "0.99", "0.98", "src-en", "_", "src-so", "_",
        ])
    }

    #[test]
    fn parses_all_nine_fields() {
        let parsed = parse_raw_line(&row([
            "Good morning",
            "Subax wanaagsan",
            "1.4321",
            "0.97",
            "0.95",
            "news",
            "http://a.example/x",
            "blog",
            "http://b.example/y",
        ]))
        .unwrap();

        assert_eq!(parsed.english, "Good morning");
        assert_eq!(parsed.somali, "Subax wanaagsan");
        assert_eq!(parsed.laser_score, 1.4321);
        assert_eq!(parsed.english_lid_score, 0.97);
        assert_eq!(parsed.somali_lid_score, 0.95);
        assert_eq!(parsed.english_source, "news");
        assert_eq!(parsed.english_url.as_deref(), Some("http://a.example/x"));
        assert_eq!(parsed.somali_url.as_deref(), Some("http://b.example/y"));
    }

    #[test]
    fn field_order_is_preserved() {
        // Distinct values in every slot must land in the matching field.
        let parsed =
            parse_raw_line(&row(["E", "S", "1.0", "0.9", "0.8", "ES", "EU", "SS", "SU"])).unwrap();
        assert_eq!(parsed.english_source, "ES");
        assert_eq!(parsed.english_url.as_deref(), Some("EU"));
        assert_eq!(parsed.somali_source, "SS");
        assert_eq!(parsed.somali_url.as_deref(), Some("SU"));
    }

    #[test]
    fn wrong_field_count_is_malformed() {
        assert_eq!(
            parse_raw_line("only\tthree\tfields"),
            Err(RowError::Malformed { found: 3 })
        );
    }

    #[test]
    fn unparsable_score_is_reported_with_its_field() {
        let line = row([
            "a", "b", "NaNsense", "0.9", "0.9", "s", "_", "s", "_",
        ]);
        assert_eq!(
            parse_raw_line(&line),
            Err(RowError::InvalidNumeric {
                field: "laser_score"
            })
        );
    }

    #[test]
    fn missing_url_token_becomes_none() {
        let parsed = parse_raw_line(&sample()).unwrap();
        assert!(parsed.english_url.is_none());
        assert!(parsed.somali_url.is_none());
    }

    #[test]
    fn trailing_newline_is_not_a_field() {
        assert_eq!(
            parse_raw_line(&format!("{}\r\n", sample())).unwrap(),
            parse_raw_line(&sample()).unwrap()
        );
    }

    #[test]
    fn record_ids_are_zero_padded_and_urls_serialize_as_null() {
        let record = build_record(&parse_raw_line(&sample()).unwrap(), 1);
        let json: serde_json::Value = serde_json::to_value(&record).unwrap();
        assert_eq!(json["id"], "nllb-eng-som-00000001");
        assert_eq!(json["language_pair"], LANGUAGE_PAIR);
        assert!(json["english_url"].is_null());
        assert!(json["somali_url"].is_null());
    }

    #[test]
    fn length_ratio_counts_characters_and_never_divides_by_zero() {
        assert_eq!(length_ratio("abcd", "ab"), 2.0);
        assert_eq!(length_ratio("", ""), 0.0);
        assert_eq!(length_ratio("aaaa", ""), 4.0);
        // Multi-byte characters count once each, not per byte.
        assert_eq!(length_ratio("éé", "é"), 2.0);
    }
}
