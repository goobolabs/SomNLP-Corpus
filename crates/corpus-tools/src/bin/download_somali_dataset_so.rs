use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::somali_dataset::download_somali_dataset;

const DEFAULT_OUTPUT: &str = "data/raw/somali-dataset/somali-dataset_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali Alpaca instruction dataset and export JSONL")]
struct Args {
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    #[command(flatten)]
    limit: LimitArgs,

    #[command(flatten)]
    stream: StreamArgs,
}

fn main() -> Result<()> {
    let args = Args::parse();

    download_somali_dataset(&args.output, args.limit.limit, args.stream.streaming())?;

    Ok(())
}
