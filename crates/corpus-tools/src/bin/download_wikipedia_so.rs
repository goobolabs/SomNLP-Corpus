use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::wikipedia::download_wikipedia;

const DEFAULT_OUTPUT: &str = "data/raw/wikipedia/wikipedia_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali Wikipedia subset and export JSONL")]
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
    
    download_wikipedia(&args.output, args.limit.limit, args.stream.streaming())?;

    Ok(())
}
