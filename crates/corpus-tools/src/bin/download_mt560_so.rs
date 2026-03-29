use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::export::export_parquet_shards;
use corpus_tools::hf::{filter_paths, HfClient};
use corpus_tools::mt560;

const DEFAULT_OUTPUT: &str = "data/raw/mt560/mt560_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download Somali sentences from OPUS MT560 and export JSONL")]
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
    let hf = HfClient::new();
    let shards = filter_paths(mt560::mt560_train_shards(), ".parquet");

    export_parquet_shards(
        &hf,
        mt560::dataset_repo(),
        &shards,
        &args.output,
        mt560::text_column(),
        args.limit.limit,
        args.stream.streaming(),
        "MT560 Somali export complete",
        &mt560::source_url(),
        Some(mt560::SOURCE_TAG),
    )?;

    Ok(())
}
