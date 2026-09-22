use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::finepdfs::download_finepdfs;

const DEFAULT_OUTPUT: &str = "data/raw/finepdfs/finepdfs_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali FinePDFs subset and export JSONL")]
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

    download_finepdfs(&args.output, args.limit.limit, args.stream.streaming())?;

    Ok(())
}
