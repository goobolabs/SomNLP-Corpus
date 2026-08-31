use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::glot500::download_glot500;

const DEFAULT_OUTPUT: &str = "data/raw/glot500/glot500_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali Glot500 subset and export JSONL")]
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

    download_glot500(&args.output, args.limit.limit, args.stream.streaming())?;

    Ok(())
}
