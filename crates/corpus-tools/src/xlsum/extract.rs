//! Stage 3 — pull `title` / `text` / `summary` (plus provenance) out of the
//! downloaded parquet shards.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};

use crate::parquet_source::iter_string_rows;
use crate::xlsum::config::{Layout, COLUMNS};
use crate::xlsum::download::Shard;
use crate::xlsum::record::XlsumRecord;

/// Extract every record from `shards`, keeping the source `id` / `url` /
/// `split` for traceability.
///
/// Values are returned exactly as stored upstream; [`crate::xlsum::clean`] is
/// what normalizes them.
pub fn extract_records(shards: &[Shard], limit: Option<u64>) -> Result<Vec<XlsumRecord>> {
    let columns: Vec<String> = COLUMNS.iter().map(|name| name.to_string()).collect();
    let mut records: Vec<XlsumRecord> = Vec::new();

    for shard in shards {
        if limit.is_some_and(|limit| records.len() as u64 >= limit) {
            break;
        }

        let rows = iter_string_rows(&shard.local_path, &columns)
            .with_context(|| format!("reading shard {}", shard.local_path.display()))?;

        let before = records.len();
        for row in rows {
            if limit.is_some_and(|limit| records.len() as u64 >= limit) {
                break;
            }
            let row = row?;
            records.push(XlsumRecord {
                id: row[0].clone(),
                url: row[1].clone(),
                split: shard.split.clone(),
                title: row[2].clone(),
                text: row[3].clone(),
                summary: row[4].clone(),
            });
        }
        println!("  {:<11} {} record(s)", shard.split, records.len() - before);
    }

    println!("Extracted {} record(s).", records.len());
    Ok(records)
}

/// Persist records as JSON Lines, one object per line.
pub fn save_records(path: &Path, records: &[XlsumRecord]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }

    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for record in records {
        writeln!(writer, "{}", serde_json::to_string(record)?)?;
    }
    writer.flush()?;

    println!("Wrote {} record(s) -> {}", records.len(), path.display());
    Ok(())
}

/// Read records previously written by [`save_records`].
pub fn load_records(path: &Path) -> Result<Vec<XlsumRecord>> {
    let file = File::open(path)
        .with_context(|| format!("{} not found — run the extract stage first", path.display()))?;

    let mut records = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: XlsumRecord = serde_json::from_str(&line)
            .with_context(|| format!("invalid JSON on line {} of {}", index + 1, path.display()))?;
        records.push(record);
    }
    Ok(records)
}

/// Load extracted records for a later stage, extracting first if the JSONL is
/// not on disk yet.
pub fn load_or_extract(
    layout: &Layout,
    shards: &[Shard],
    limit: Option<u64>,
) -> Result<Vec<XlsumRecord>> {
    let path = layout.extracted();
    if path.exists() {
        return load_records(&path);
    }
    println!("No {} — extracting from the raw shards.", path.display());
    extract_records(shards, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<XlsumRecord> {
        vec![XlsumRecord {
            id: "abc".into(),
            url: "https://bbc.com/somali/1".into(),
            split: "train".into(),
            title: "Cinwaan".into(),
            text: "Qoraal dheer oo Soomaali ah.".into(),
            summary: "Kooban".into(),
        }]
    }

    #[test]
    fn jsonl_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("extracted.jsonl");
        save_records(&path, &sample()).unwrap();
        assert_eq!(load_records(&path).unwrap(), sample());
    }

    #[test]
    fn blank_lines_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("extracted.jsonl");
        std::fs::write(&path, "\n{\"text\":\"hal\"}\n\n").unwrap();
        let records = load_records(&path).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].text, "hal");
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(load_records(Path::new("does/not/exist.jsonl")).is_err());
    }
}
