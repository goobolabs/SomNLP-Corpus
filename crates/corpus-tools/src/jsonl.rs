use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::json;

use crate::stats::Stats;

pub struct JsonlWriter {
    writer: BufWriter<File>,
    progress: ProgressBar,
    pub stats: Stats,
}

impl JsonlWriter {
    pub fn create(path: &Path, label: &str) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }

        let file = File::create(path)
            .with_context(|| format!("creating output file {}", path.display()))?;
        let progress = ProgressBar::new_spinner();
        progress.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg} {pos} docs")
                .context("progress bar template")?
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        progress.set_message(label.to_string());

        Ok(Self {
            writer: BufWriter::new(file),
            progress,
            stats: Stats::default(),
        })
    }

    pub fn write_text(&mut self, text: &str) -> Result<()> {
        let line = serde_json::to_string(&json!({ "text": text }))?;
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.stats.record(text);
        self.progress.inc(1);
        Ok(())
    }

    pub fn write_text_source(&mut self, source: &str, text: &str) -> Result<()> {
        let line = serde_json::to_string(&json!({ "text": text }))?;
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.stats.record_source(source, text);
        self.progress.inc(1);
        Ok(())
    }

    pub fn write_tagged(&mut self, text: &str, source: &str) -> Result<()> {
        let line = serde_json::to_string(&json!({ "text": text, "source": source }))?;
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.stats.record(text);
        self.progress.inc(1);
        Ok(())
    }

    pub fn finish(self) {
        self.progress.finish_and_clear();
    }
}

pub fn is_non_empty(text: &str) -> bool {
    !text.trim().is_empty()
}

pub fn read_jsonl_texts(path: &Path) -> Result<impl Iterator<Item = Result<String>> + '_> {
    use std::io::{BufRead, BufReader};

    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = BufReader::new(file);

    Ok(reader.lines().enumerate().filter_map(move |(index, line)| {
        match line {
            Ok(raw) => {
                let raw = raw.trim();
                if raw.is_empty() {
                    return None;
                }
                let text = match parse_jsonl_text(raw, path, index + 1) {
                    Ok(text) => text,
                    Err(_) => return None,
                };
                if text.is_empty() {
                    return None;
                }
                Some(Ok(text))
            }
            Err(error) => Some(Err(error.into())),
        }
    }))
}

fn parse_jsonl_text(line: &str, path: &Path, line_number: usize) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(line)
        .with_context(|| format!("invalid JSON on line {line_number} of {}", path.display()))?;
    let text = value
        .get("text")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    Ok(text)
}
