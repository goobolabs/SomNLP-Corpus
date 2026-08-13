//! Official NLLB English–Somali downloader (`allenai/nllb`, `eng_Latn-som_Latn`).
//!
//! The pair ships as a single gzip-compressed nine-column TSV on the AllenAI
//! bucket, so there is nothing to list or shard: download that one file, stream
//! it through the score filters, and export JSONL.
//!
//! Rust port of the standalone `download_nllb_somali.py`. The download half is
//! kept whole — resume, redirect host pinning, gzip verification — while the
//! export follows this crate's convention of raw source text, leaving cleaning
//! to the corpus-pipeline.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::ValueEnum;
use common::hash::content_hash;
use flate2::read::MultiGzDecoder;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_RANGE, LOCATION, RANGE};
use reqwest::{StatusCode, Url};
use serde_json::json;

use crate::cli::print_export_summary;
use crate::stats::format_number;
use crate::Stats;

pub const SOURCE_TAG: &str = "nllb";
pub const DATASET_REPO: &str = "allenai/nllb";
pub const LANGUAGE_PAIR: &str = "eng_Latn-som_Latn";

/// Taken from the official `allenai/nllb` loader metadata:
/// `https://storage.googleapis.com/allennlp-data-bucket/nllb/{pair}.gz`.
pub const OFFICIAL_URL: &str =
    "https://storage.googleapis.com/allennlp-data-bucket/nllb/eng_Latn-som_Latn.gz";

/// File name the archive keeps on disk, matching the remote object.
pub const RAW_FILENAME: &str = "eng_Latn-som_Latn.gz";

/// Hosts the bucket is served from. A redirect leaving this set aborts the
/// download instead of quietly fetching the corpus from somewhere else.
const OFFICIAL_HOSTS: [&str; 2] = [
    "storage.googleapis.com",
    "allennlp-data-bucket.storage.googleapis.com",
];

const REDIRECT_CODES: [u16; 5] = [301, 302, 303, 307, 308];
const MAX_REDIRECTS: usize = 5;
const CHUNK_SIZE: usize = 1024 * 1024;

/// Raw TSV columns, in order. The file has no header, so a row that does not
/// split into exactly this many fields is unusable.
const NUM_FIELDS: usize = 9;

/// Placeholder the corpus uses for a missing provenance URL.
const MISSING_URL: &str = "_";

/// Human-readable provenance line for the export summary.
pub fn source_url() -> String {
    format!("{OFFICIAL_URL} ({DATASET_REPO}, {LANGUAGE_PAIR})")
}

/// What the tool produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// One `{"text": <somali>}` record per pair — the corpus format.
    Jsonl,
    /// Full parallel record: both sides plus scores and provenance.
    Pairs,
    /// Download the `.gz` archive only, no conversion.
    Raw,
}

/// Score and length thresholds. Every threshold defaults to `None`, which keeps
/// each valid pair: the pipeline, not this tool, decides what is corpus-worthy.
#[derive(Debug, Default, Clone, Copy)]
pub struct Filters {
    pub min_laser: Option<f64>,
    pub min_eng_lid: Option<f64>,
    pub min_som_lid: Option<f64>,
    pub min_chars: Option<usize>,
    pub max_length_ratio: Option<f64>,
}

impl Filters {
    /// Reject thresholds that can never match, so a typo fails before the
    /// download rather than after it silently exports nothing.
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("--min-eng-lid", self.min_eng_lid),
            ("--min-som-lid", self.min_som_lid),
        ] {
            if let Some(value) = value {
                if !(0.0..=1.0).contains(&value) {
                    bail!("{name} must be within [0, 1], got {value}");
                }
            }
        }
        if self.max_length_ratio.is_some_and(|ratio| ratio < 1.0) {
            bail!("--max-length-ratio must be >= 1");
        }
        Ok(())
    }

    /// Whether a parsed row survives the configured thresholds.
    pub fn keep(&self, row: &Row) -> bool {
        if row.english.is_empty() || row.somali.is_empty() {
            return false;
        }
        if self.min_laser.is_some_and(|min| row.laser_score < min) {
            return false;
        }
        if self
            .min_eng_lid
            .is_some_and(|min| row.english_lid_score < min)
        {
            return false;
        }
        if self
            .min_som_lid
            .is_some_and(|min| row.somali_lid_score < min)
        {
            return false;
        }
        if self
            .min_chars
            .is_some_and(|min| row.somali.chars().count() < min)
        {
            return false;
        }
        if self
            .max_length_ratio
            .is_some_and(|max| length_ratio(&row.english, &row.somali) > max)
        {
            return false;
        }
        true
    }
}

/// Everything [`download_nllb`] needs, assembled by the binary from its CLI.
#[derive(Debug, Clone)]
pub struct Options {
    pub output: PathBuf,
    /// Where the `.gz` lands. Defaults to the output directory.
    pub archive_dir: Option<PathBuf>,
    pub format: Format,
    pub limit: Option<u64>,
    pub filters: Filters,
    /// Drop repeated Somali sentences.
    pub dedup: bool,
    pub overwrite: bool,
    pub delete_archive: bool,
    pub source_url: String,
    pub progress: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            output: PathBuf::from("data/raw/nllb/nllb_so.jsonl"),
            archive_dir: None,
            format: Format::Jsonl,
            limit: None,
            filters: Filters::default(),
            dedup: false,
            overwrite: false,
            delete_archive: false,
            source_url: OFFICIAL_URL.to_string(),
            progress: true,
        }
    }
}

impl Options {
    pub fn validate(&self) -> Result<()> {
        self.filters.validate()?;
        if !is_official_host(&self.source_url) {
            bail!(
                "--source-url must point at the official bucket: {}",
                self.source_url
            );
        }
        Ok(())
    }

    fn archive_path(&self) -> PathBuf {
        let dir = match &self.archive_dir {
            Some(dir) => dir.clone(),
            None => self
                .output
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
        };
        dir.join(RAW_FILENAME)
    }
}

/// Row tallies for one run, reported after the export.
#[derive(Debug, Default, Clone)]
pub struct Counts {
    pub rows: u64,
    pub malformed: u64,
    pub filtered: u64,
    pub duplicates: u64,
    pub written: u64,
    pub stats: Stats,
}

/// Download the English–Somali archive and export it as JSONL.
pub fn download_nllb(options: &Options) -> Result<Counts> {
    options.validate()?;
    let archive = options.archive_path();

    println!(
        "Downloading {DATASET_REPO} ({LANGUAGE_PAIR}) from {}",
        options.source_url
    );
    download_archive(
        &options.source_url,
        &archive,
        options.overwrite,
        options.progress,
    )?;

    if options.format == Format::Raw {
        println!("Raw archive ready at {}", archive.display());
        return Ok(Counts::default());
    }

    let counts = convert(options, &archive)?;
    if counts.stats.total_docs == 0 {
        bail!("no pairs exported from {DATASET_REPO}; check the filter thresholds");
    }

    println!();
    println!("Rows read      : {}", format_number(counts.rows));
    println!("Malformed rows : {}", format_number(counts.malformed));
    println!("Filtered rows  : {}", format_number(counts.filtered));
    println!("Duplicate rows : {}", format_number(counts.duplicates));
    print_export_summary(
        "NLLB English–Somali export complete",
        &counts.stats,
        &options.output,
        &source_url(),
    );

    if options.delete_archive {
        fs::remove_file(&archive).with_context(|| format!("deleting {}", archive.display()))?;
        println!("Deleted archive : {}", archive.display());
    }

    Ok(counts)
}

// --------------------------------------------------------------------------
// Conversion
// --------------------------------------------------------------------------

/// Stream the archive through parse -> filter -> dedup and write the export.
fn convert(options: &Options, archive: &Path) -> Result<Counts> {
    let file = File::open(archive).with_context(|| format!("opening {}", archive.display()))?;
    let reader = BufReader::with_capacity(
        CHUNK_SIZE,
        MultiGzDecoder::new(BufReader::with_capacity(CHUNK_SIZE, file)),
    );

    let mut writer = RecordWriter::create(&options.output, options.format)?;
    let mut counts = Counts::default();
    let mut seen: HashSet<String> = HashSet::new();

    for line in reader.split(b'\n') {
        let line = line.context("reading the NLLB archive")?;
        // The corpus is UTF-8, but a truncated shard must not abort the run.
        let line = String::from_utf8_lossy(&line);
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        if options.limit.is_some_and(|limit| counts.written >= limit) {
            break;
        }

        counts.rows += 1;
        let Some(row) = Row::parse(line) else {
            counts.malformed += 1;
            continue;
        };
        if !options.filters.keep(&row) {
            counts.filtered += 1;
            continue;
        }
        // Dedup on the Somali side: that is the text the corpus keeps, and the
        // same sentence often appears against several English translations.
        if options.dedup && !seen.insert(content_hash(&row.somali).0) {
            counts.duplicates += 1;
            continue;
        }

        writer.write(&row)?;
        counts.written += 1;
    }

    counts.stats = writer.finish()?;
    Ok(counts)
}

/// One raw TSV row.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
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

impl Row {
    /// Parse a raw line, or `None` when the field count or a score is unusable.
    pub fn parse(line: &str) -> Option<Self> {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != NUM_FIELDS {
            return None;
        }
        Some(Self {
            english: fields[0].to_string(),
            somali: fields[1].to_string(),
            laser_score: parse_score(fields[2])?,
            english_lid_score: parse_score(fields[3])?,
            somali_lid_score: parse_score(fields[4])?,
            english_source: fields[5].to_string(),
            english_url: clean_url(fields[6]),
            somali_source: fields[7].to_string(),
            somali_url: clean_url(fields[8]),
        })
    }
}

fn parse_score(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|score| score.is_finite())
}

fn clean_url(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value == MISSING_URL {
        None
    } else {
        Some(value.to_string())
    }
}

fn length_ratio(english: &str, somali: &str) -> f64 {
    let english = english.chars().count();
    let somali = somali.chars().count();
    english.max(somali) as f64 / english.min(somali).max(1) as f64
}

/// JSONL writer that stages to `<output>.part` and only renames into place on a
/// clean finish, so an interrupted run never leaves a half-written export.
struct RecordWriter {
    path: PathBuf,
    part: PathBuf,
    writer: Option<BufWriter<File>>,
    progress: ProgressBar,
    format: Format,
    stats: Stats,
}

impl RecordWriter {
    fn create(path: &Path, format: Format) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }
        let part = part_path(path);
        let file = File::create(&part).with_context(|| format!("creating {}", part.display()))?;

        let progress = ProgressBar::new_spinner();
        progress.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg} {pos} pairs")
                .context("progress bar template")?
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        progress.set_draw_target(ProgressDrawTarget::stderr());
        progress.set_message("Writing");

        Ok(Self {
            path: path.to_path_buf(),
            part,
            writer: Some(BufWriter::with_capacity(CHUNK_SIZE, file)),
            progress,
            format,
            stats: Stats::default(),
        })
    }

    fn write(&mut self, row: &Row) -> Result<()> {
        let record = match self.format {
            Format::Pairs => json!({
                "text": row.somali,
                "english": row.english,
                "laser_score": row.laser_score,
                "english_lid_score": row.english_lid_score,
                "somali_lid_score": row.somali_lid_score,
                "english_source": row.english_source,
                "english_url": row.english_url,
                "somali_source": row.somali_source,
                "somali_url": row.somali_url,
                "language_pair": LANGUAGE_PAIR,
            }),
            // Raw never reaches a writer; treat it like the plain corpus format.
            Format::Jsonl | Format::Raw => json!({ "text": row.somali }),
        };

        let writer = self
            .writer
            .as_mut()
            .expect("writer is only taken by finish()");
        writer.write_all(serde_json::to_string(&record)?.as_bytes())?;
        writer.write_all(b"\n")?;

        self.stats.record(&row.somali);
        self.progress.inc(1);
        Ok(())
    }

    /// Flush, rename the part file into place, and hand back the export stats.
    fn finish(mut self) -> Result<Stats> {
        let mut writer = self
            .writer
            .take()
            .expect("finish() consumes the writer exactly once");
        writer.flush().context("flushing the export")?;
        writer
            .into_inner()?
            .sync_all()
            .context("syncing the export")?;
        self.progress.finish_and_clear();

        fs::rename(&self.part, &self.path).with_context(|| {
            format!(
                "renaming {} to {}",
                self.part.display(),
                self.path.display()
            )
        })?;
        Ok(self.stats.clone())
    }
}

impl Drop for RecordWriter {
    fn drop(&mut self) {
        // Still holding the writer means finish() never ran: the export failed
        // partway, so drop the partial file rather than leaving it behind.
        if self.writer.take().is_some() {
            let _ = fs::remove_file(&self.part);
        }
    }
}

// --------------------------------------------------------------------------
// Download
// --------------------------------------------------------------------------

/// Download the archive, resuming a previous `.part` when one is present.
fn download_archive(url: &str, dest: &Path, overwrite: bool, show_progress: bool) -> Result<()> {
    if !is_official_host(url) {
        bail!("refusing non-official host: {url}");
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating archive directory {}", parent.display()))?;
    }

    if dest.exists() && !overwrite {
        if verify_gzip(dest) {
            println!(
                "Archive already present, skipping download: {}",
                dest.display()
            );
            return Ok(());
        }
        eprintln!(
            "Existing archive is not valid gzip, re-downloading: {}",
            dest.display()
        );
    }

    let part = part_path(dest);
    if overwrite && part.exists() {
        fs::remove_file(&part).with_context(|| format!("removing {}", part.display()))?;
    }
    let mut resume_from = part.metadata().map(|meta| meta.len()).unwrap_or(0);

    let client = build_client()?;
    let mut response = resolve(&client, url, resume_from)?;

    let mut append = true;
    let total = match response.status() {
        StatusCode::PARTIAL_CONTENT => range_total(&response, resume_from),
        StatusCode::OK => {
            if resume_from > 0 {
                eprintln!("Server ignored the range request, restarting the download");
                resume_from = 0;
                append = false;
            }
            response.content_length()
        }
        // The range covers the whole object: the part file is already complete.
        StatusCode::RANGE_NOT_SATISFIABLE => {
            if part.exists() {
                fs::rename(&part, dest)?;
            }
            if !verify_gzip(dest) {
                bail!(
                    "completed archive failed gzip verification: {}",
                    dest.display()
                );
            }
            return Ok(());
        }
        status => bail!("unexpected status {status} from {url}"),
    };

    let progress = show_progress
        .then(|| byte_progress(total, resume_from))
        .transpose()?;

    let mut open_options = OpenOptions::new();
    open_options.create(true).write(true);
    if append {
        open_options.append(true);
    } else {
        open_options.truncate(true);
    }
    let mut file = open_options
        .open(&part)
        .with_context(|| format!("opening {}", part.display()))?;

    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut downloaded = resume_from;
    loop {
        let read = response
            .read(&mut buffer)
            .context("reading the response body")?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .with_context(|| format!("writing {}", part.display()))?;
        downloaded += read as u64;
        if let Some(bar) = &progress {
            bar.set_position(downloaded);
        }
    }
    file.flush()?;
    file.sync_all()?;
    if let Some(bar) = progress {
        bar.finish_and_clear();
    }

    if let Some(total) = total {
        let actual = part.metadata()?.len();
        if actual != total {
            bail!(
                "size mismatch: expected {total} bytes, got {actual}; \
                 partial download left at {}",
                part.display()
            );
        }
    }
    if !verify_gzip(&part) {
        bail!(
            "download failed gzip verification; left at {}",
            part.display()
        );
    }

    fs::rename(&part, dest)
        .with_context(|| format!("renaming {} to {}", part.display(), dest.display()))?;
    println!("Archive ready at {}", dest.display());
    Ok(())
}

fn build_client() -> Result<Client> {
    Client::builder()
        .user_agent("corpus-tools/0.1")
        .connect_timeout(Duration::from_secs(30))
        // The blocking client's default 30s budget covers the whole request,
        // which a multi-gigabyte body will always blow through.
        .timeout(None)
        // Redirects are followed by hand so every hop can be host-checked.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("building the HTTP client")
}

/// Follow redirects manually, refusing any hop that leaves the official bucket.
fn resolve(client: &Client, url: &str, resume_from: u64) -> Result<Response> {
    let mut current = url.to_string();

    for _ in 0..=MAX_REDIRECTS {
        let mut request = client.get(&current);
        if resume_from > 0 {
            request = request.header(RANGE, format!("bytes={resume_from}-"));
        }
        let response = request
            .send()
            .with_context(|| format!("requesting {current}"))?;

        if !REDIRECT_CODES.contains(&response.status().as_u16()) {
            return Ok(response);
        }

        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| anyhow!("redirect from {current} without a Location header"))?;
        let next = Url::parse(&current)
            .context("parsing the current URL")?
            .join(location)
            .context("resolving the redirect target")?;
        if !is_official_host(next.as_str()) {
            bail!("refusing redirect to non-official host: {next}");
        }
        current = next.to_string();
    }

    bail!("too many redirects while resolving {url}")
}

/// Total object size for a partial response, from `Content-Range` if the server
/// sent one and from the resumed offset plus `Content-Length` otherwise.
fn range_total(response: &Response, resume_from: u64) -> Option<u64> {
    let from_header = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_content_range);
    from_header.or_else(|| response.content_length().map(|len| resume_from + len))
}

fn parse_content_range(header: &str) -> Option<u64> {
    header.rsplit('/').next()?.trim().parse().ok()
}

pub fn is_official_host(url: &str) -> bool {
    let Ok(url) = Url::parse(url) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    OFFICIAL_HOSTS.contains(&host.as_str()) || host.ends_with(".storage.googleapis.com")
}

/// Confirm the file is gzip that actually decompresses, not an error page or a
/// truncated transfer that happened to have the right byte count.
fn verify_gzip(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    if file.metadata().map(|meta| meta.len()).unwrap_or(0) == 0 {
        return false;
    }

    let mut decoder = MultiGzDecoder::new(BufReader::new(file));
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut decompressed = 0usize;
    while decompressed < buffer.len() {
        match decoder.read(&mut buffer[decompressed..]) {
            Ok(0) => break,
            Ok(read) => decompressed += read,
            Err(_) => return false,
        }
    }
    decompressed > 0
}

fn byte_progress(total: Option<u64>, initial: u64) -> Result<ProgressBar> {
    let progress = ProgressBar::new(total.unwrap_or(0));
    progress.set_style(
        ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes}")
            .context("progress template")?
            .progress_chars("=>-"),
    );
    progress.set_message("Downloading");
    progress.set_position(initial);
    Ok(progress)
}

fn part_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROW: &str = "Hello world\tSalaan adduunka\t1.25\t0.98\t0.91\tcommon_crawl\t\
                       https://example.com/en\tparacrawl\t_";

    #[test]
    fn parses_a_full_row() {
        let row = Row::parse(ROW).expect("valid row");
        assert_eq!(row.english, "Hello world");
        assert_eq!(row.somali, "Salaan adduunka");
        assert_eq!(row.laser_score, 1.25);
        assert_eq!(row.english_url.as_deref(), Some("https://example.com/en"));
        // "_" is the corpus placeholder for "no URL recorded".
        assert_eq!(row.somali_url, None);
    }

    #[test]
    fn rejects_wrong_field_counts() {
        assert!(Row::parse("only\ttwo").is_none());
        assert!(Row::parse(&format!("{ROW}\textra")).is_none());
    }

    #[test]
    fn rejects_unparsable_scores() {
        assert!(Row::parse(&ROW.replace("1.25", "n/a")).is_none());
        assert!(Row::parse(&ROW.replace("1.25", "nan")).is_none());
    }

    #[test]
    fn default_thresholds_keep_every_non_empty_pair() {
        assert!(Filters::default().keep(&Row::parse(ROW).unwrap()));
    }

    #[test]
    fn empty_side_is_always_dropped() {
        let row = Row::parse(&ROW.replace("Salaan adduunka", "")).unwrap();
        assert!(!Filters::default().keep(&row));
    }

    #[test]
    fn thresholds_filter_low_scores() {
        let row = Row::parse(ROW).unwrap();
        let mut filters = Filters::default();
        filters.min_laser = Some(1.5);
        assert!(!filters.keep(&row));
        filters.min_laser = Some(1.0);
        assert!(filters.keep(&row));
        filters.min_som_lid = Some(0.95);
        assert!(!filters.keep(&row));
    }

    #[test]
    fn length_ratio_uses_characters() {
        assert_eq!(length_ratio("abcd", "áéíó"), 1.0);
        assert_eq!(length_ratio("abcdef", "abc"), 2.0);
    }

    #[test]
    fn out_of_range_lid_is_a_usage_error() {
        let filters = Filters {
            min_som_lid: Some(1.5),
            ..Filters::default()
        };
        assert!(filters.validate().is_err());
    }

    #[test]
    fn official_hosts_are_pinned() {
        assert!(is_official_host(OFFICIAL_URL));
        assert!(is_official_host(
            "https://allennlp-data-bucket.storage.googleapis.com/nllb/x.gz"
        ));
        assert!(!is_official_host("https://evil.example.com/nllb/x.gz"));
        // Suffix matching must not accept a lookalike domain.
        assert!(!is_official_host(
            "https://storage.googleapis.com.evil.net/x.gz"
        ));
    }

    #[test]
    fn unofficial_source_url_is_rejected() {
        let options = Options {
            source_url: "https://evil.example.com/x.gz".to_string(),
            ..Options::default()
        };
        assert!(options.validate().is_err());
    }

    #[test]
    fn archive_lands_next_to_the_output_by_default() {
        let options = Options {
            output: PathBuf::from("data/raw/nllb/nllb_so.jsonl"),
            ..Options::default()
        };
        assert_eq!(
            options.archive_path(),
            PathBuf::from("data/raw/nllb").join(RAW_FILENAME)
        );
    }

    #[test]
    fn archive_dir_overrides_the_output_location() {
        let options = Options {
            archive_dir: Some(PathBuf::from("scratch")),
            ..Options::default()
        };
        assert_eq!(
            options.archive_path(),
            PathBuf::from("scratch").join(RAW_FILENAME)
        );
    }

    #[test]
    fn content_range_yields_the_total_size() {
        assert_eq!(parse_content_range("bytes 100-199/12345"), Some(12345));
        assert_eq!(parse_content_range("bytes 100-199/*"), None);
    }

    #[test]
    fn part_path_appends_a_suffix() {
        assert_eq!(
            part_path(Path::new("data/raw/nllb/nllb_so.jsonl")),
            PathBuf::from("data/raw/nllb/nllb_so.jsonl.part")
        );
    }
}
