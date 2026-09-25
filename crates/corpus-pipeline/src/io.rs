//! Shared streaming JSONL I/O for pipeline stages: record readers/writers, a
//! reject-sidecar writer, and a stats-report writer.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use common::types::{CorpusRecord, RawRecord};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Stream JSON records of type `T` from a JSONL file, skipping blank lines.
pub fn read_jsonl<T: DeserializeOwned>(path: &Path) -> Result<impl Iterator<Item = Result<T>>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = BufReader::new(file);
    let path = path.to_path_buf();
    Ok(reader
        .lines()
        .enumerate()
        .filter_map(move |(index, line)| match line {
            Ok(raw) => {
                if raw.trim().is_empty() {
                    return None;
                }
                let parsed = serde_json::from_str::<T>(&raw).with_context(|| {
                    format!("invalid JSON on line {} of {}", index + 1, path.display())
                });
                Some(parsed)
            }
            Err(error) => Some(Err(error.into())),
        }))
}

/// Records per batch handed to the rayon pool by [`par_map_jsonl`].
pub const PAR_BATCH: usize = 16_384;

/// Parallel counterpart of a `read_jsonl` + per-record loop. Reads up to `limit`
/// non-blank lines in batches; each batch is parsed and passed through `map` on the
/// rayon pool, then every result goes to `sink` sequentially in input order. A
/// producer thread prepares the next batch while `sink` drains the current one. As
/// long as `map` is a pure per-record function, output is identical to the serial loop.
pub fn par_map_jsonl<T, U, M, S>(path: &Path, limit: Option<u64>, map: M, mut sink: S) -> Result<()>
where
    T: DeserializeOwned,
    U: Send,
    M: Fn(T) -> U + Sync,
    S: FnMut(U) -> Result<()>,
{
    use rayon::prelude::*;
    use std::sync::mpsc::sync_channel;

    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut lines = BufReader::new(file).lines().enumerate();
    let map = &map;

    std::thread::scope(|scope| {
        let (tx, rx) = sync_channel::<Result<Vec<Result<U>>>>(2);
        scope.spawn(move || {
            let mut taken = 0u64;
            loop {
                let mut batch: Vec<(usize, String)> = Vec::with_capacity(PAR_BATCH);
                while batch.len() < PAR_BATCH && !limit.is_some_and(|limit| taken >= limit) {
                    let Some((index, line)) = lines.next() else {
                        break;
                    };
                    let raw = match line {
                        Ok(raw) => raw,
                        Err(error) => {
                            let _ = tx.send(Err(error.into()));
                            return;
                        }
                    };
                    if raw.trim().is_empty() {
                        continue;
                    }
                    batch.push((index, raw));
                    taken += 1;
                }
                if batch.is_empty() {
                    return;
                }

                let results: Vec<Result<U>> = batch
                    .into_par_iter()
                    .map(|(index, raw)| {
                        let parsed = serde_json::from_str::<T>(&raw).with_context(|| {
                            format!("invalid JSON on line {} of {}", index + 1, path.display())
                        })?;
                        Ok(map(parsed))
                    })
                    .collect();
                // A closed channel means the sink failed; stop producing.
                if tx.send(Ok(results)).is_err() {
                    return;
                }
            }
        });

        for batch in rx {
            for result in batch? {
                sink(result?)?;
            }
        }
        Ok(())
    })
}

/// Serialize a record to one JSONL line (without the trailing newline), so stages
/// can serialize on the rayon pool and hand pre-built lines to the writers.
pub fn to_line<T: Serialize>(record: &T) -> Result<String> {
    Ok(serde_json::to_string(record)?)
}

/// Convenience reader for raw merge records.
pub fn read_raw(path: &Path) -> Result<impl Iterator<Item = Result<RawRecord>>> {
    read_jsonl(path)
}

/// Convenience reader for processed corpus records.
pub fn read_corpus(path: &Path) -> Result<impl Iterator<Item = Result<CorpusRecord>>> {
    read_jsonl(path)
}

/// Buffered JSONL writer for serializable records.
pub struct JsonlSink {
    writer: BufWriter<File>,
    count: u64,
}

impl JsonlSink {
    pub fn create(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
        Ok(Self {
            writer: BufWriter::new(file),
            count: 0,
        })
    }

    pub fn write<T: Serialize>(&mut self, record: &T) -> Result<()> {
        self.write_line(&to_line(record)?)
    }

    /// Write a line produced by [`to_line`].
    pub fn write_line(&mut self, line: &str) -> Result<()> {
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.count += 1;
        Ok(())
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn finish(mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

/// Lazily-created sidecar writer for rejected records. The file is only created
/// once the first rejected record is written.
pub struct RejectWriter {
    path: PathBuf,
    sink: Option<JsonlSink>,
}

impl RejectWriter {
    pub fn new(path: PathBuf) -> Self {
        Self { path, sink: None }
    }

    /// Derive the reject sidecar path for a primary output (`x.jsonl` ->
    /// `x.rejected.jsonl`).
    pub fn for_output(output: &Path) -> Self {
        let mut name = output
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "output".to_string());
        name.push_str(".rejected.jsonl");
        let path = output.with_file_name(name);
        Self::new(path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&mut self, record: &CorpusRecord) -> Result<()> {
        self.write_line(&to_line(record)?)
    }

    /// Write a line produced by [`to_line`].
    pub fn write_line(&mut self, line: &str) -> Result<()> {
        if self.sink.is_none() {
            self.sink = Some(JsonlSink::create(&self.path)?);
        }
        self.sink.as_mut().unwrap().write_line(line)
    }

    pub fn count(&self) -> u64 {
        self.sink.as_ref().map(JsonlSink::count).unwrap_or(0)
    }

    pub fn finish(self) -> Result<()> {
        if let Some(sink) = self.sink {
            sink.finish()?;
        }
        Ok(())
    }
}

/// Write a stats report as pretty JSON under `reports/`.
pub fn write_report<T: Serialize>(path: &Path, report: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(report)?;
    fs::write(path, json).with_context(|| format!("writing report {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::types::RawRecord;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_raw_records() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("raw.jsonl");
        let mut sink = JsonlSink::create(&path).unwrap();
        sink.write(&RawRecord {
            text: "hello".into(),
            source: Some("hplt".into()),
        })
        .unwrap();
        sink.finish().unwrap();

        let records: Vec<RawRecord> = read_raw(&path).unwrap().map(Result::unwrap).collect();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].text, "hello");
        assert_eq!(records[0].source.as_deref(), Some("hplt"));
    }

    #[test]
    fn reject_sidecar_path_derivation() {
        let writer = RejectWriter::for_output(Path::new("data/cleaned/cleaned_so.jsonl"));
        assert!(writer
            .path
            .ends_with("data/cleaned/cleaned_so.rejected.jsonl"));
    }
}
