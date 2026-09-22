use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::somali_tinystories::{self, download_somali_tinystories};

const DEFAULT_OUTPUT: &str = "data/raw/somali-tinystories/somali-tinystories_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali TinyStories dataset and export JSONL")]
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

    println!("Downloading {}", somali_tinystories::DATASET_NAME);

    download_somali_tinystories(&args.output, args.limit.limit, args.stream.streaming())?;

    Ok(())
}
