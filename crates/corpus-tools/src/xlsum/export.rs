//! Stage 5 — write the cleaned corpus as JSONL, CSV and plain text.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};

use crate::xlsum::config::Layout;
use crate::xlsum::record::XlsumRecord;

/// Column order of the CSV export.
pub const CSV_FIELDS: [&str; 6] = ["id", "url", "split", "title", "summary", "text"];

/// UTF-8 byte order mark, written so Excel opens the CSV as UTF-8.
const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// One of the three corpus formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Jsonl,
    Csv,
    Txt,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Jsonl => "jsonl",
            Format::Csv => "csv",
            Format::Txt => "txt",
        }
    }
}

/// Write one JSON object per line.
pub fn export_jsonl(path: &Path, records: &[XlsumRecord]) -> Result<()> {
    let mut writer = create(path)?;
    for record in records {
        writeln!(writer, "{}", serde_json::to_string(record)?)?;
    }
    finish(writer, path, records.len(), "JSONL")
}

/// Write the corpus as CSV (UTF-8 with BOM, RFC 4180 quoting).
pub fn export_csv(path: &Path, records: &[XlsumRecord]) -> Result<()> {
    let mut writer = create(path)?;
    writer.write_all(UTF8_BOM)?;
    writeln!(writer, "{}", CSV_FIELDS.join(","))?;

    for record in records {
        let row: Vec<String> = CSV_FIELDS
            .iter()
            .map(|field| csv_escape(csv_value(record, field)))
            .collect();
        writeln!(writer, "{}", row.join(","))?;
    }
    finish(writer, path, records.len(), "CSV")
}

/// Write one cleaned document (`text`) per line.
///
/// Cleaning already collapsed internal newlines, so every article occupies
/// exactly one line.
pub fn export_txt(path: &Path, records: &[XlsumRecord]) -> Result<()> {
    let mut writer = create(path)?;
    for record in records {
        writeln!(writer, "{}", record.text)?;
    }
    finish(writer, path, records.len(), "plain-text")
}

/// Write the requested formats into the layout's corpus paths.
pub fn export_formats(layout: &Layout, records: &[XlsumRecord], formats: &[Format]) -> Result<()> {
    layout.ensure_dirs()?;
    for format in formats {
        match format {
            Format::Jsonl => export_jsonl(&layout.corpus_jsonl(), records)?,
            Format::Csv => export_csv(&layout.corpus_csv(), records)?,
            Format::Txt => export_txt(&layout.corpus_txt(), records)?,
        }
    }
    Ok(())
}

/// Write all three formats.
pub fn export_all(layout: &Layout, records: &[XlsumRecord]) -> Result<()> {
    export_formats(layout, records, &[Format::Jsonl, Format::Csv, Format::Txt])
}

fn csv_value<'a>(record: &'a XlsumRecord, field: &str) -> &'a str {
    match field {
        "id" => &record.id,
        "url" => &record.url,
        "split" => &record.split,
        "title" => &record.title,
        "summary" => &record.summary,
        "text" => &record.text,
        _ => "",
    }
}

/// Quote a CSV field when it holds a comma, quote, or line break; doubling any
/// embedded quotes.
fn csv_escape(value: &str) -> String {
    let needs_quotes = value.contains([',', '"', '\n', '\r']);
    if !needs_quotes {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn create(path: &Path) -> Result<BufWriter<File>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    Ok(BufWriter::new(file))
}

fn finish(mut writer: BufWriter<File>, path: &Path, count: usize, label: &str) -> Result<()> {
    writer.flush()?;
    println!("Wrote {count} {label} record(s) -> {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records() -> Vec<XlsumRecord> {
        vec![XlsumRecord {
            id: "1".into(),
            url: "https://example.test/1".into(),
            split: "train".into(),
            title: "Cinwaan, oo \"xigasho\" leh".into(),
            text: "Qoraal dheer oo Soomaali ah.".into(),
            summary: "Kooban".into(),
        }]
    }

    #[test]
    fn plain_values_are_not_quoted() {
        assert_eq!(csv_escape("Soomaaliya"), "Soomaaliya");
    }

    #[test]
    fn commas_and_quotes_are_escaped() {
        assert_eq!(
            csv_escape("Cinwaan, oo \"xigasho\" leh"),
            "\"Cinwaan, oo \"\"xigasho\"\" leh\""
        );
    }

    #[test]
    fn csv_starts_with_a_bom_and_header() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corpus.csv");
        export_csv(&path, &records()).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(UTF8_BOM));
        let body = String::from_utf8(bytes[3..].to_vec()).unwrap();
        let mut lines = body.lines();
        assert_eq!(lines.next().unwrap(), "id,url,split,title,summary,text");
        assert!(lines.next().unwrap().contains("\"\"xigasho\"\""));
    }

    #[test]
    fn txt_writes_one_document_per_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corpus.txt");
        export_txt(&path, &records()).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body.lines().count(), 1);
        assert_eq!(body.trim_end(), "Qoraal dheer oo Soomaali ah.");
    }

    #[test]
    fn jsonl_keeps_somali_characters_unescaped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corpus.jsonl");
        export_jsonl(&path, &records()).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("Soomaali"));
        assert_eq!(body.lines().count(), 1);
    }
}
