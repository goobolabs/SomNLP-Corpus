use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::glot::{self, download_glot};

const DEFAULT_OUTPUT: &str = "data/raw/glot/glot_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali Glot500 (som_Latn) subset and export JSONL")]
struct Args {
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    #[arg(long, default_value = glot::DEFAULT_SPLIT)]
    split: String,

    #[command(flatten)]
    limit: LimitArgs,

    #[command(flatten)]
    stream: StreamArgs,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!(
        "Downloading {} (config: {}, split: {})",
        glot::DATASET_NAME,
        glot::DATASET_CONFIG,
        args.split
    );

    download_glot(
        &args.output,
        &args.split,
        args.limit.limit,
        args.stream.streaming(),
    )?;

    Ok(())
}
