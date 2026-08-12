//! Orchestration: download → convert → audit.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;

use crate::nllb::audit::{render_markdown, Audit, AuditReport};
use crate::nllb::config::{is_official_host, official_url, source_url, Layout};
use crate::nllb::download::{download_archive, DownloadOptions};
use crate::nllb::export::{write_atomic, CorpusWriter, Format, PartFile};
use crate::nllb::filter::Rejection;
use crate::nllb::process::{process_stream, ProcessOptions, ProcessSummary};
use crate::stats::format_number;

/// Everything one run needs.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Archive URL; must stay on an official host.
    pub source_url: String,
    pub format: Format,
    /// Write the audit report, the Markdown summary and the sample file.
    pub audit: bool,
    pub sample_size: usize,
    pub download: DownloadOptions,
    /// Delete the `.gz` after a successful conversion (never for `raw`).
    pub delete_archive: bool,
    pub process: ProcessOptions,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            source_url: official_url(),
            format: Format::Raw,
            audit: false,
            sample_size: 20,
            download: DownloadOptions::default(),
            delete_archive: false,
            process: ProcessOptions::default(),
        }
    }
}

/// What a run produced.
#[derive(Debug, Clone)]
pub struct RunSummary {
    pub raw_path: PathBuf,
    pub raw_size: Option<u64>,
    /// Files written by the converter and the audit, in write order.
    pub outputs: Vec<PathBuf>,
    pub process: ProcessSummary,
    pub report: Option<AuditReport>,
}

/// Download the archive and convert it into `options.format`.
pub fn run(layout: &Layout, options: &RunOptions) -> Result<RunSummary> {
    options.process.filter.validate()?;
    if !is_official_host(&options.source_url) {
        bail!("--source-url must point to an official host: {}", options.source_url);
    }

    layout.ensure_dirs()?;

    println!("Source : {}", source_url());
    println!("Output : {}", layout.root().display());
    println!("Format : {}", options.format.as_str());
    println!();

    let raw_path = download_archive(&options.source_url, &layout.raw_archive(), &options.download)?;
    let raw_size = fs::metadata(&raw_path).map(|meta| meta.len()).ok();

    if options.format == Format::Raw && !options.audit {
        println!("Raw archive ready at {}", raw_path.display());
        return Ok(RunSummary {
            raw_path,
            raw_size,
            outputs: Vec::new(),
            process: ProcessSummary::default(),
            report: None,
        });
    }

    let mut audit = match options.audit {
        true => Some(Audit::new(options.sample_size)?),
        false => None,
    };
    let mut writer = CorpusWriter::create(options.format, layout)?;

    let outcome = gzip_lines(&raw_path).and_then(|lines| {
        process_stream(lines, &options.process, writer.as_mut(), audit.as_mut())
    });

    let summary = match outcome {
        Ok(summary) => summary,
        Err(error) => {
            // A failed conversion leaves no half-written output behind.
            if let Some(writer) = writer {
                writer.abort();
            }
            return Err(error);
        }
    };

    let mut outputs = match writer {
        Some(writer) => writer.commit()?,
        None => Vec::new(),
    };

    let report = match audit.as_mut() {
        Some(audit) => {
            let (report, written) = write_audit(layout, audit, raw_size)?;
            outputs.extend(written);
            Some(report)
        }
        None => None,
    };

    print_summary(&summary, &outputs, layout);

    if options.delete_archive {
        if options.format == Format::Raw {
            println!("--delete-archive ignored for --format raw");
        } else {
            fs::remove_file(&raw_path)
                .with_context(|| format!("deleting {}", raw_path.display()))?;
            println!("Deleted raw archive: {}", raw_path.display());
        }
    }

    Ok(RunSummary {
        raw_path,
        raw_size,
        outputs,
        process: summary,
        report,
    })
}

/// Write `audit_report.json`, `audit_report.md` and `sample_pairs.jsonl`.
pub fn write_audit(
    layout: &Layout,
    audit: &mut Audit,
    raw_size: Option<u64>,
) -> Result<(AuditReport, Vec<PathBuf>)> {
    let report = audit.to_report(raw_size);

    let json = serde_json::to_string_pretty(&report)? + "\n";
    let json_path = write_atomic(&layout.audit_json(), &json)?;
    let markdown_path = write_atomic(&layout.audit_md(), &render_markdown(&report))?;

    let mut samples = PartFile::create(&layout.samples())?;
    for record in audit.sampler().records() {
        writeln!(samples, "{}", serde_json::to_string(&record)?)?;
    }
    let samples_path = samples.commit()?;

    println!("Audit report  -> {}", json_path.display());
    println!("Audit summary -> {}", markdown_path.display());
    println!("Samples       -> {}", samples_path.display());

    Ok((report, vec![json_path, markdown_path, samples_path]))
}

/// Stream a gzip archive line by line, replacing invalid UTF-8 rather than
/// failing: one bad byte sequence must not abort a multi-gigabyte conversion.
pub fn gzip_lines(path: &Path) -> Result<impl Iterator<Item = Result<String>>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    Ok(LossyLines {
        reader: BufReader::new(GzDecoder::new(file)),
        buffer: Vec::new(),
    })
}

struct LossyLines<R: BufRead> {
    reader: R,
    buffer: Vec<u8>,
}

impl<R: BufRead> Iterator for LossyLines<R> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        self.buffer.clear();
        match self.reader.read_until(b'\n', &mut self.buffer) {
            Ok(0) => None,
            Ok(_) => Some(Ok(String::from_utf8_lossy(&self.buffer).into_owned())),
            Err(error) => Some(Err(error).context("reading the decompressed archive")),
        }
    }
}

fn print_summary(summary: &ProcessSummary, outputs: &[PathBuf], layout: &Layout) {
    println!();
    println!("{}", "=".repeat(48));
    println!("NLLB English-Somali conversion complete");
    println!("{}", "=".repeat(48));
    println!("Lines inspected : {}", format_number(summary.total_lines));
    println!("Valid rows      : {}", format_number(summary.valid_parsed));
    println!("Accepted rows   : {}", format_number(summary.accepted));
    println!("Malformed rows  : {}", format_number(summary.malformed));
    println!(
        "Invalid numeric : {}",
        format_number(summary.invalid_numeric)
    );

    let rejected: Vec<(Rejection, u64)> = summary
        .rejections
        .iter()
        .filter(|(_, count)| *count > 0)
        .collect();
    if !rejected.is_empty() {
        println!("Rejected rows   : {}", format_number(summary.rejections.total()));
        for (reason, count) in rejected {
            println!("  - {:<26}: {}", reason.label(), format_number(count));
        }
    }

    println!("Output directory: {}", layout.root().display());
    for path in outputs {
        println!("  - {}", path.display());
    }
    println!("Source          : {}", source_url());
    println!("{}", "=".repeat(48));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nllb::filter::FilterOptions;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    fn raw(english: &str, somali: &str) -> String {
        [english, somali, "1.2000", "0.99", "0.98", "src-en", "_", "src-so", "_"].join("\t")
    }

    /// Write an archive where the real download would land, so `run` finds a
    /// valid file and skips the network entirely.
    fn seed_archive(layout: &Layout, lines: &[String]) {
        layout.ensure_dirs().unwrap();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        for line in lines {
            writeln!(encoder, "{line}").unwrap();
        }
        fs::write(layout.raw_archive(), encoder.finish().unwrap()).unwrap();
    }

    fn options(format: Format) -> RunOptions {
        RunOptions {
            format,
            download: DownloadOptions {
                show_progress: false,
                ..DownloadOptions::default()
            },
            process: ProcessOptions {
                progress: false,
                ..ProcessOptions::default()
            },
            ..RunOptions::default()
        }
    }

    #[test]
    fn lossy_lines_survive_invalid_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.gz");
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(b"good\n\xff\xfe bad\n").unwrap();
        fs::write(&path, encoder.finish().unwrap()).unwrap();

        let lines: Vec<String> = gzip_lines(&path).unwrap().map(Result::unwrap).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "good\n");
        assert!(lines[1].contains('\u{FFFD}'));
    }

    #[test]
    fn converts_a_cached_archive_to_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        seed_archive(&layout, &[raw("hello", "salaan"), raw("bye", "nabadgelyo")]);

        let summary = run(&layout, &options(Format::Jsonl)).unwrap();
        assert_eq!(summary.process.accepted, 2);
        assert_eq!(summary.outputs, vec![layout.jsonl()]);
        assert_eq!(
            fs::read_to_string(layout.jsonl()).unwrap().lines().count(),
            2
        );
        assert!(summary.report.is_none());
    }

    #[test]
    fn raw_format_only_keeps_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        seed_archive(&layout, &[raw("hello", "salaan")]);

        let summary = run(&layout, &options(Format::Raw)).unwrap();
        assert!(summary.outputs.is_empty());
        assert!(layout.raw_archive().exists());
        assert!(!layout.jsonl().exists());
    }

    #[test]
    fn audit_writes_all_three_report_files() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        seed_archive(
            &layout,
            &[raw("hello", "salaan"), raw("", "salaan"), raw("bye", "nabadgelyo")],
        );

        let summary = run(
            &layout,
            &RunOptions {
                audit: true,
                sample_size: 2,
                ..options(Format::Jsonl)
            },
        )
        .unwrap();

        let report = summary.report.unwrap();
        assert_eq!(report.accepted_rows, 2);
        assert_eq!(report.rejected_rows_by_reason.get("empty English"), Some(1));
        assert_eq!(report.raw_compressed_size_bytes, summary.raw_size);

        assert!(layout.audit_json().exists());
        assert!(layout.audit_md().exists());
        assert!(!fs::read_to_string(layout.samples()).unwrap().is_empty());

        let written: AuditReport =
            serde_json::from_str(&fs::read_to_string(layout.audit_json()).unwrap()).unwrap();
        assert_eq!(written, report);
    }

    #[test]
    fn delete_archive_runs_only_after_a_successful_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        seed_archive(&layout, &[raw("hello", "salaan")]);

        run(
            &layout,
            &RunOptions {
                delete_archive: true,
                ..options(Format::Somali)
            },
        )
        .unwrap();

        assert!(!layout.raw_archive().exists());
        assert_eq!(fs::read_to_string(layout.somali_txt()).unwrap(), "salaan\n");
    }

    #[test]
    fn raw_format_keeps_the_archive_even_with_delete_archive() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        seed_archive(&layout, &[raw("hello", "salaan")]);

        run(
            &layout,
            &RunOptions {
                delete_archive: true,
                audit: true,
                ..options(Format::Raw)
            },
        )
        .unwrap();
        assert!(layout.raw_archive().exists());
    }

    #[test]
    fn impossible_thresholds_fail_before_downloading() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());

        let error = run(
            &layout,
            &RunOptions {
                process: ProcessOptions {
                    filter: FilterOptions {
                        max_length_ratio: Some(0.5),
                        ..FilterOptions::default()
                    },
                    progress: false,
                    ..ProcessOptions::default()
                },
                ..options(Format::Jsonl)
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("--max-length-ratio"));
        assert!(!layout.raw_archive().exists());
    }

    #[test]
    fn unofficial_source_urls_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let error = run(
            &Layout::new(dir.path()),
            &RunOptions {
                source_url: "https://evil.example/x.gz".into(),
                ..options(Format::Jsonl)
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("official host"));
    }
}
