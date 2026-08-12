//! Output formats, written through `.part` files that are renamed into place
//! only after a successful run.
//!
//! An interrupted or failed conversion therefore never leaves a truncated
//! final file behind: either the whole output is there, or none of it is.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::nllb::config::{Layout, FIELD_NAMES, LANGUAGE_PAIR};
use crate::nllb::record::{build_record, RawRow};

/// Suffix of the in-progress file.
pub const PART_SUFFIX: &str = ".part";

/// One output format. `Raw` keeps the downloaded archive and writes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Raw,
    Jsonl,
    Csv,
    Tsv,
    Somali,
    Parallel,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Raw => "raw",
            Format::Jsonl => "jsonl",
            Format::Csv => "csv",
            Format::Tsv => "tsv",
            Format::Somali => "somali",
            Format::Parallel => "parallel",
        }
    }
}

/// A file being written under a temporary name.
pub struct PartFile {
    final_path: PathBuf,
    part_path: PathBuf,
    writer: BufWriter<File>,
}

impl PartFile {
    pub fn create(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        let part_path = with_part_suffix(path);
        let file = File::create(&part_path)
            .with_context(|| format!("creating {}", part_path.display()))?;
        Ok(Self {
            final_path: path.to_path_buf(),
            part_path,
            writer: BufWriter::new(file),
        })
    }

    pub fn path(&self) -> &Path {
        &self.final_path
    }

    /// Flush, fsync, and rename the part file over the final path.
    pub fn commit(mut self) -> Result<PathBuf> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        drop(self.writer);
        fs::rename(&self.part_path, &self.final_path).with_context(|| {
            format!(
                "renaming {} to {}",
                self.part_path.display(),
                self.final_path.display()
            )
        })?;
        Ok(self.final_path)
    }

    /// Discard the partial output; the final path is left untouched.
    pub fn abort(self) {
        drop(self.writer);
        let _ = fs::remove_file(&self.part_path);
    }
}

impl Write for PartFile {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

/// Writes accepted pairs in one of the converted formats.
pub enum CorpusWriter {
    Jsonl(PartFile),
    /// CSV and TSV differ only in their delimiter.
    Delimited { part: PartFile, delimiter: char },
    Somali(PartFile),
    Parallel { english: PartFile, somali: PartFile },
}

impl CorpusWriter {
    /// Open the writer for `format`, or `None` for [`Format::Raw`].
    pub fn create(format: Format, layout: &Layout) -> Result<Option<Self>> {
        let writer = match format {
            Format::Raw => return Ok(None),
            Format::Jsonl => CorpusWriter::Jsonl(PartFile::create(&layout.jsonl())?),
            Format::Csv => CorpusWriter::delimited(&layout.csv(), ',')?,
            Format::Tsv => CorpusWriter::delimited(&layout.tsv(), '\t')?,
            Format::Somali => CorpusWriter::Somali(PartFile::create(&layout.somali_txt())?),
            Format::Parallel => CorpusWriter::Parallel {
                english: PartFile::create(&layout.parallel_en())?,
                somali: PartFile::create(&layout.parallel_so())?,
            },
        };
        Ok(Some(writer))
    }

    fn delimited(path: &Path, delimiter: char) -> Result<Self> {
        let mut part = PartFile::create(path)?;
        let header: Vec<&str> = std::iter::once("id")
            .chain(FIELD_NAMES)
            .chain(std::iter::once("language_pair"))
            .collect();
        writeln!(part, "{}", header.join(&delimiter.to_string()))?;
        Ok(CorpusWriter::Delimited { part, delimiter })
    }

    /// Write the `index`-th accepted pair (1-based).
    pub fn write(&mut self, row: &RawRow, index: u64) -> Result<()> {
        match self {
            CorpusWriter::Jsonl(part) => {
                writeln!(part, "{}", serde_json::to_string(&build_record(row, index))?)?;
            }
            CorpusWriter::Delimited { part, delimiter } => {
                let record = build_record(row, index);
                let fields = [
                    record.id,
                    record.english,
                    record.somali,
                    format_score(record.laser_score),
                    format_score(record.english_lid_score),
                    format_score(record.somali_lid_score),
                    record.english_source,
                    record.english_url.unwrap_or_default(),
                    record.somali_source,
                    record.somali_url.unwrap_or_default(),
                    LANGUAGE_PAIR.to_string(),
                ];
                let row: Vec<String> = fields
                    .iter()
                    .map(|value| escape(value, *delimiter))
                    .collect();
                writeln!(part, "{}", row.join(&delimiter.to_string()))?;
            }
            CorpusWriter::Somali(part) => writeln!(part, "{}", row.somali)?,
            CorpusWriter::Parallel { english, somali } => {
                // Both sides are written together, so the two files stay
                // line-synchronized however many rows were rejected.
                writeln!(english, "{}", row.english)?;
                writeln!(somali, "{}", row.somali)?;
            }
        }
        Ok(())
    }

    /// Rename every part file into place.
    pub fn commit(self) -> Result<Vec<PathBuf>> {
        Ok(match self {
            CorpusWriter::Jsonl(part) | CorpusWriter::Somali(part) => vec![part.commit()?],
            CorpusWriter::Delimited { part, .. } => vec![part.commit()?],
            CorpusWriter::Parallel { english, somali } => {
                vec![english.commit()?, somali.commit()?]
            }
        })
    }

    /// Drop every part file, leaving no output behind.
    pub fn abort(self) {
        match self {
            CorpusWriter::Jsonl(part) | CorpusWriter::Somali(part) => part.abort(),
            CorpusWriter::Delimited { part, .. } => part.abort(),
            CorpusWriter::Parallel { english, somali } => {
                english.abort();
                somali.abort();
            }
        }
    }
}

/// Write `contents` atomically to `path`.
pub fn write_atomic(path: &Path, contents: &str) -> Result<PathBuf> {
    let mut part = PartFile::create(path)?;
    part.write_all(contents.as_bytes())?;
    part.commit()
}

fn with_part_suffix(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(PART_SUFFIX);
    PathBuf::from(name)
}

/// Render a score the way the JSONL export does, so the flat formats and the
/// JSONL agree digit for digit.
fn format_score(value: f64) -> String {
    let mut buffer = serde_json::to_string(&value).unwrap_or_else(|_| value.to_string());
    if buffer == "null" {
        // Non-finite scores have no JSON form; keep the Rust rendering.
        buffer = value.to_string();
    }
    buffer
}

/// Quote a field that holds the delimiter, a quote, or a line break, doubling
/// any embedded quotes (RFC 4180).
fn escape(value: &str, delimiter: char) -> String {
    let needs_quotes = value.contains([delimiter, '"', '\n', '\r']);
    if !needs_quotes {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nllb::record::parse_raw_line;

    fn row(english: &str, somali: &str) -> RawRow {
        parse_raw_line(&[english, somali, "1.2", "0.99", "0.98", "src-en", "_", "src-so", "_"].join("\t"))
            .unwrap()
    }

    fn layout(dir: &tempfile::TempDir) -> Layout {
        Layout::new(dir.path())
    }

    #[test]
    fn plain_values_are_not_quoted() {
        assert_eq!(escape("Soomaaliya", ','), "Soomaaliya");
        assert_eq!(escape("a,b", '\t'), "a,b");
    }

    #[test]
    fn delimiters_and_quotes_are_escaped() {
        assert_eq!(escape("a,b", ','), "\"a,b\"");
        assert_eq!(escape("a\tb", '\t'), "\"a\tb\"");
        assert_eq!(escape("say \"hi\"", ','), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn output_appears_only_after_commit() {
        let dir = tempfile::tempdir().unwrap();
        let layout = layout(&dir);
        let mut writer = CorpusWriter::create(Format::Jsonl, &layout).unwrap().unwrap();
        writer.write(&row("hello", "salaan"), 1).unwrap();

        assert!(!layout.jsonl().exists());
        assert!(with_part_suffix(&layout.jsonl()).exists());

        writer.commit().unwrap();
        assert!(layout.jsonl().exists());
        assert!(!with_part_suffix(&layout.jsonl()).exists());
    }

    #[test]
    fn abort_leaves_no_output_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let layout = layout(&dir);
        let mut writer = CorpusWriter::create(Format::Jsonl, &layout).unwrap().unwrap();
        writer.write(&row("hello", "salaan"), 1).unwrap();
        writer.abort();

        assert!(!layout.jsonl().exists());
        assert!(!with_part_suffix(&layout.jsonl()).exists());
    }

    #[test]
    fn jsonl_keeps_somali_characters_and_null_urls() {
        let dir = tempfile::tempdir().unwrap();
        let layout = layout(&dir);
        let somali = "Sancadaha Soomaaliyeed — waa qurux badan";
        let mut writer = CorpusWriter::create(Format::Jsonl, &layout).unwrap().unwrap();
        writer.write(&row("Culture", somali), 1).unwrap();
        writer.commit().unwrap();

        let body = fs::read_to_string(layout.jsonl()).unwrap();
        let value: serde_json::Value = serde_json::from_str(body.lines().next().unwrap()).unwrap();
        assert_eq!(value["somali"], somali);
        assert!(value["english_url"].is_null());
    }

    #[test]
    fn csv_starts_with_the_field_header() {
        let dir = tempfile::tempdir().unwrap();
        let layout = layout(&dir);
        let somali = "Qaxwe iyo shaah — subax wanaagsan";
        let mut writer = CorpusWriter::create(Format::Csv, &layout).unwrap().unwrap();
        writer.write(&row("Coffee", somali), 1).unwrap();
        writer.commit().unwrap();

        let body = fs::read_to_string(layout.csv()).unwrap();
        let mut lines = body.lines();
        assert_eq!(
            lines.next().unwrap(),
            "id,english,somali,laser_score,english_lid_score,somali_lid_score,\
             english_source,english_url,somali_source,somali_url,language_pair"
        );
        let columns: Vec<&str> = lines.next().unwrap().split(',').collect();
        assert_eq!(columns[2], somali);
        // A missing URL is an empty column, never the "_" placeholder.
        assert_eq!(columns[7], "");
    }

    #[test]
    fn tsv_uses_tabs_and_keeps_the_same_columns() {
        let dir = tempfile::tempdir().unwrap();
        let layout = layout(&dir);
        let mut writer = CorpusWriter::create(Format::Tsv, &layout).unwrap().unwrap();
        writer.write(&row("Coffee", "Qaxwe"), 1).unwrap();
        writer.commit().unwrap();

        let body = fs::read_to_string(layout.tsv()).unwrap();
        let columns: Vec<&str> = body.lines().nth(1).unwrap().split('\t').collect();
        assert_eq!(columns.len(), 11);
        assert_eq!(columns[1], "Coffee");
        assert_eq!(columns[2], "Qaxwe");
    }

    #[test]
    fn parallel_files_stay_line_synchronized() {
        let dir = tempfile::tempdir().unwrap();
        let layout = layout(&dir);
        let mut writer = CorpusWriter::create(Format::Parallel, &layout).unwrap().unwrap();
        for index in 0..5u64 {
            writer
                .write(&row(&format!("line {index}"), &format!("sadar {index}")), index + 1)
                .unwrap();
        }
        writer.commit().unwrap();

        let english = fs::read_to_string(layout.parallel_en()).unwrap();
        let somali = fs::read_to_string(layout.parallel_so()).unwrap();
        assert_eq!(english.lines().count(), 5);
        assert_eq!(english.lines().count(), somali.lines().count());
        for (english, somali) in english.lines().zip(somali.lines()) {
            assert!(english.starts_with("line"));
            assert!(somali.starts_with("sadar"));
        }
    }

    #[test]
    fn somali_export_is_one_sentence_per_line() {
        let dir = tempfile::tempdir().unwrap();
        let layout = layout(&dir);
        let mut writer = CorpusWriter::create(Format::Somali, &layout).unwrap().unwrap();
        writer.write(&row("hello", "salaan"), 1).unwrap();
        writer.write(&row("bye", "nabadgelyo"), 2).unwrap();
        writer.commit().unwrap();

        let body = fs::read_to_string(layout.somali_txt()).unwrap();
        assert_eq!(body.lines().collect::<Vec<_>>(), vec!["salaan", "nabadgelyo"]);
    }

    #[test]
    fn raw_format_opens_no_writer() {
        let dir = tempfile::tempdir().unwrap();
        assert!(CorpusWriter::create(Format::Raw, &layout(&dir))
            .unwrap()
            .is_none());
    }
}
