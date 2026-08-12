//! Download, inspect, filter and convert the official NLLB English–Somali
//! corpus (`allenai/nllb`, `eng_Latn-som_Latn`).
//!
//! Only the single English–Somali archive is fetched, straight from the
//! official AllenAI bucket. Default thresholds keep every valid non-empty row;
//! the values in the examples below are starting points, not quality
//! guarantees.
//!
//! ```text
//! # keep the raw archive only
//! download_nllb_so --format raw
//!
//! # filtered JSONL with an audit report
//! download_nllb_so --format jsonl --min-laser 1.05 --min-eng-lid 0.90 \
//!     --min-som-lid 0.90 --normalize --deduplicate --audit
//!
//! # cleaned Somali-only text
//! download_nllb_so --format somali --min-laser 1.05 --min-som-lid 0.90 \
//!     --min-som-chars 20 --normalize --deduplicate-somali --audit
//! ```

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use corpus_tools::nllb::config::{official_url, Layout, DEFAULT_ROOT};
use corpus_tools::nllb::download::DownloadOptions;
use corpus_tools::nllb::export::Format;
use corpus_tools::nllb::filter::FilterOptions;
use corpus_tools::nllb::pipeline::{run, RunOptions};
use corpus_tools::nllb::process::ProcessOptions;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    /// Keep the downloaded archive and convert nothing.
    Raw,
    /// One JSON object per pair, with scores and provenance.
    Jsonl,
    Csv,
    Tsv,
    /// Somali sentences only, one per line.
    Somali,
    /// Line-synchronized `.en` / `.so` translation files.
    Parallel,
}

impl OutputFormat {
    fn format(self) -> Format {
        match self {
            OutputFormat::Raw => Format::Raw,
            OutputFormat::Jsonl => Format::Jsonl,
            OutputFormat::Csv => Format::Csv,
            OutputFormat::Tsv => Format::Tsv,
            OutputFormat::Somali => Format::Somali,
            OutputFormat::Parallel => Format::Parallel,
        }
    }
}

#[derive(Debug, Parser)]
#[command(about = "Download, filter and convert the NLLB English-Somali corpus")]
struct Args {
    /// Directory for the archive and every generated file.
    #[arg(long, default_value = DEFAULT_ROOT)]
    output_dir: PathBuf,

    /// Output format to produce.
    #[arg(long, value_enum, default_value = "raw")]
    format: OutputFormat,

    /// Process only the first N valid rows (for smoke tests).
    #[arg(long)]
    max_rows: Option<u64>,

    /// Minimum LASER alignment score.
    #[arg(long)]
    min_laser: Option<f64>,

    /// Minimum English language-ID score, within [0, 1].
    #[arg(long)]
    min_eng_lid: Option<f64>,

    /// Minimum Somali language-ID score, within [0, 1].
    #[arg(long)]
    min_som_lid: Option<f64>,

    /// Minimum English length in characters.
    #[arg(long)]
    min_eng_chars: Option<usize>,

    /// Minimum Somali length in characters.
    #[arg(long)]
    min_som_chars: Option<usize>,

    /// Maximum English length in characters.
    #[arg(long)]
    max_eng_chars: Option<usize>,

    /// Maximum Somali length in characters.
    #[arg(long)]
    max_som_chars: Option<usize>,

    /// Maximum English/Somali character-length ratio (>= 1.0).
    #[arg(long)]
    max_length_ratio: Option<f64>,

    /// Drop exact duplicate English-Somali pairs.
    #[arg(long)]
    deduplicate: bool,

    /// Drop duplicate Somali sentences.
    #[arg(long)]
    deduplicate_somali: bool,

    /// Apply conservative Unicode/whitespace normalization.
    #[arg(long)]
    normalize: bool,

    /// Write an audit report and deterministic samples.
    #[arg(long)]
    audit: bool,

    /// Number of random sample pairs kept by the audit.
    #[arg(long, default_value_t = 20)]
    sample_size: usize,

    /// Re-download even if a valid archive is already present.
    #[arg(long)]
    overwrite: bool,

    /// Delete the archive after a successful conversion.
    #[arg(long)]
    delete_archive: bool,

    /// Override the source URL. Must stay on an official host.
    #[arg(long, default_value_t = official_url())]
    source_url: String,

    /// Disable progress bars.
    #[arg(long)]
    no_progress: bool,
}

impl Args {
    fn filter(&self) -> FilterOptions {
        FilterOptions {
            min_laser: self.min_laser,
            min_eng_lid: self.min_eng_lid,
            min_som_lid: self.min_som_lid,
            min_eng_chars: self.min_eng_chars,
            min_som_chars: self.min_som_chars,
            max_eng_chars: self.max_eng_chars,
            max_som_chars: self.max_som_chars,
            max_length_ratio: self.max_length_ratio,
        }
    }

    fn options(&self) -> RunOptions {
        let progress = !self.no_progress;
        RunOptions {
            source_url: self.source_url.clone(),
            format: self.format.format(),
            audit: self.audit,
            sample_size: self.sample_size,
            download: DownloadOptions {
                overwrite: self.overwrite,
                show_progress: progress,
                ..DownloadOptions::default()
            },
            delete_archive: self.delete_archive,
            process: ProcessOptions {
                filter: self.filter(),
                normalize: self.normalize,
                deduplicate: self.deduplicate,
                deduplicate_somali: self.deduplicate_somali,
                max_rows: self.max_rows,
                progress,
            },
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.deduplicate || args.deduplicate_somali {
        println!(
            "Note: deduplication keeps one digest per kept row in memory; the full \
             archive needs several GB."
        );
    }

    run(&Layout::new(args.output_dir.clone()), &args.options())?;
    Ok(())
}
