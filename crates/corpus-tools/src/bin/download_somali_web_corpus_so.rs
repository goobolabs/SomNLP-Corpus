use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::somali_web_corpus::download_somali_web_corpus;

const DEFAULT_OUTPUT: &str = "data/raw/somali-web-corpus/somali-web-corpus_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali Web Corpus dataset and export JSONL")]
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

    download_somali_web_corpus(&args.output, args.limit.limit, args.stream.streaming())?;

    Ok(())
}
