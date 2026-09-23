use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::fineweb2::download_fineweb2;

const DEFAULT_OUTPUT: &str = "data/raw/fineweb-2/fineweb-2_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali FineWeb-2 subset and export JSONL")]
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

    download_fineweb2(&args.output, args.limit.limit, args.stream.streaming())?;

    Ok(())
}
