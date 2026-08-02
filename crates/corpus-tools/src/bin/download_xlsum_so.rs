use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::xlsum::{self, download_xlsum};

const DEFAULT_OUTPUT: &str = "data/raw/xlsum/xlsum_so.jsonl";

/// Which XL-Sum column to export as the record text.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum Field {
    /// Full article body (default; the monolingual corpus signal).
    Text,
    /// Human-written abstractive summary.
    Summary,
    /// Article headline.
    Title,
}

impl Field {
    fn column(self) -> &'static str {
        match self {
            Field::Text => "text",
            Field::Summary => "summary",
            Field::Title => "title",
        }
    }
}

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

#[derive(Debug, Parser)]
#[command(about = "Download the Somali XL-Sum subset and export JSONL")]
struct Args {
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    /// Column exported as the record text.
    #[arg(long, value_enum, default_value = "text")]
    field: Field,

    /// Splits to export, in the given order.
    #[arg(long, value_enum, num_args = 1.., default_values = ["train", "validation", "test"])]
    splits: Vec<Split>,

    #[command(flatten)]
    limit: LimitArgs,

    #[command(flatten)]
    stream: StreamArgs,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let splits: Vec<String> = args
        .splits
        .iter()
        .map(|split| split.name().to_string())
        .collect();

    println!(
        "Downloading {} (config: {}) — splits: {}, field: {}",
        xlsum::DATASET_NAME,
        xlsum::DATASET_CONFIG,
        splits.join(", "),
        args.field.column()
    );

    download_xlsum(
        &args.output,
        args.field.column(),
        &splits,
        args.limit.limit,
        args.stream.streaming(),
    )?;

    Ok(())
}
