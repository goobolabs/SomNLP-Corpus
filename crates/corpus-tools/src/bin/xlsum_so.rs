//! Staged XL-Sum Somali toolkit: run one stage at a time, or all six.
//!
//! For the plain single-field JSONL export used by the merge pipeline, see the
//! `download_xlsum_so` binary instead.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use corpus_tools::xlsum::clean::{clean_records, CleanOptions};
use corpus_tools::xlsum::config::{default_splits, Layout, DEFAULT_ROOT};
use corpus_tools::xlsum::download::fetch_shards;
use corpus_tools::xlsum::export::{export_formats, Format};
use corpus_tools::xlsum::extract::{extract_records, load_records, save_records};
use corpus_tools::xlsum::inspect::inspect_shards;
use corpus_tools::xlsum::pipeline::{run_pipeline, PipelineOptions};
use corpus_tools::xlsum::stats::{generate_report, TOKENS_PER_WORD};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Split {
    Train,
    Validation,
    Test,
}

impl Split {
    fn name(self) -> &'static str {
        match self {
            Split::Train => "train",
            Split::Validation => "validation",
            Split::Test => "test",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Jsonl,
    Csv,
    Txt,
}

impl OutputFormat {
    fn format(self) -> Format {
        match self {
            OutputFormat::Jsonl => Format::Jsonl,
            OutputFormat::Csv => Format::Csv,
            OutputFormat::Txt => Format::Txt,
        }
    }
}

#[derive(Debug, Args)]
struct SplitArgs {
    /// Splits to process, in the given order.
    #[arg(long, value_enum, num_args = 1.., default_values = ["train", "validation", "test"])]
    splits: Vec<Split>,
}

impl SplitArgs {
    fn names(&self) -> Vec<String> {
        if self.splits.is_empty() {
            return default_splits();
        }
        self.splits
            .iter()
            .map(|split| split.name().to_string())
            .collect()
    }
}

#[derive(Debug, Args)]
struct CleanArgs {
    /// Minimum cleaned text length (characters) to keep a record.
    #[arg(long, default_value_t = 20)]
    min_text_chars: usize,

    /// Minimum cleaned title length (characters) to keep a record.
    #[arg(long, default_value_t = 1)]
    min_title_chars: usize,

    /// Keep records whose summary is empty.
    #[arg(long)]
    allow_empty_summary: bool,
}

impl CleanArgs {
    fn options(&self) -> CleanOptions {
        CleanOptions {
            min_text_chars: self.min_text_chars,
            min_title_chars: self.min_title_chars,
            require_summary: !self.allow_empty_summary,
            ..CleanOptions::default()
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    about = "Download, clean and export the Somali XL-Sum corpus",
    long_about = None
)]
struct Cli {
    /// Root directory for every artefact (raw shards, corpus, reports).
    #[arg(long, global = true, default_value = DEFAULT_ROOT)]
    root: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// [1] Fetch the parquet shards into <root>/raw.
    Download {
        #[command(flatten)]
        splits: SplitArgs,

        /// Re-download shards that are already cached.
        #[arg(long)]
        force: bool,
    },

    /// [2] Report schema, row counts and sample records.
    Inspect {
        #[command(flatten)]
        splits: SplitArgs,

        /// Records to preview (0 disables previews).
        #[arg(long, default_value_t = 3)]
        samples: usize,

        /// Split to preview from.
        #[arg(long, value_enum, default_value = "train")]
        preview_split: Split,
    },

    /// [3] Extract title/text/summary into <root>/extracted.jsonl.
    Extract {
        #[command(flatten)]
        splits: SplitArgs,

        /// Stop after N records.
        #[arg(long)]
        limit: Option<u64>,
    },

    /// [4] Clean and validate <root>/extracted.jsonl in place.
    Clean {
        #[command(flatten)]
        clean: CleanArgs,
    },

    /// [5] Export the cleaned corpus to JSONL / CSV / plain text.
    Export {
        /// Formats to write.
        #[arg(long, value_enum, num_args = 1.., default_values = ["jsonl", "csv", "txt"])]
        formats: Vec<OutputFormat>,

        #[command(flatten)]
        clean: CleanArgs,
    },

    /// [6] Write the corpus metadata report.
    Stats {
        /// Tokens-per-word multiplier for the token estimate.
        #[arg(long, default_value_t = TOKENS_PER_WORD)]
        tokens_per_word: f64,
    },

    /// Run every stage end to end.
    All {
        #[command(flatten)]
        splits: SplitArgs,

        /// Re-download shards that are already cached.
        #[arg(long)]
        force: bool,

        /// Records to preview during the inspect stage.
        #[arg(long, default_value_t = 3)]
        samples: usize,

        /// Stop after N records.
        #[arg(long)]
        limit: Option<u64>,

        #[command(flatten)]
        clean: CleanArgs,

        /// Tokens-per-word multiplier for the token estimate.
        #[arg(long, default_value_t = TOKENS_PER_WORD)]
        tokens_per_word: f64,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let layout = Layout::new(cli.root.clone());

    match cli.command {
        Command::Download { splits, force } => {
            fetch_shards(&layout, &splits.names(), force)?;
        }

        Command::Inspect {
            splits,
            samples,
            preview_split,
        } => {
            let shards = fetch_shards(&layout, &splits.names(), false)?;
            inspect_shards(
                &layout.schema_report(),
                &shards,
                samples,
                preview_split.name(),
            )?;
        }

        Command::Extract { splits, limit } => {
            let shards = fetch_shards(&layout, &splits.names(), false)?;
            let records = extract_records(&shards, limit)?;
            save_records(&layout.extracted(), &records)?;
        }

        Command::Clean { clean } => {
            let records = load_records(&layout.extracted())?;
            let (cleaned, _drops) = clean_records(&records, &clean.options());
            save_records(&layout.extracted(), &cleaned)?;
        }

        Command::Export { formats, clean } => {
            let records = load_records(&layout.extracted())?;
            let (cleaned, _drops) = clean_records(&records, &clean.options());
            let formats: Vec<Format> = formats.iter().map(|format| format.format()).collect();
            export_formats(&layout, &cleaned, &formats)?;
        }

        Command::Stats { tokens_per_word } => {
            let records = load_records(&layout.extracted())?;
            generate_report(&layout.stats_report(), &records, tokens_per_word)?;
        }

        Command::All {
            splits,
            force,
            samples,
            limit,
            clean,
            tokens_per_word,
        } => {
            let options = PipelineOptions {
                splits: splits.names(),
                force,
                samples,
                limit,
                clean: clean.options(),
                tokens_per_word,
                ..PipelineOptions::default()
            };
            run_pipeline(&layout, &options)?;
        }
    }

    Ok(())
}
