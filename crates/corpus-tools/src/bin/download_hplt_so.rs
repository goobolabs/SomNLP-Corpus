use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::export::export_parquet_shards;
use corpus_tools::hf::{filter_paths, HfClient};

const DATASET_NAME: &str = "HPLT/HPLT2.0_cleaned";
const DATASET_CONFIG: &str = "som_Latn";
const DEFAULT_OUTPUT: &str = "data/raw/hplt/hplt_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali HPLT v2 subset and export JSONL")]
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
    let shards = filter_paths(hf.list_files(DATASET_NAME, DATASET_CONFIG)?, ".parquet");

    export_parquet_shards(
        &hf,
        DATASET_NAME,
        &shards,
        &args.output,
        "text",
        args.limit.limit,
        args.stream.streaming(),
        "HPLT Somali export complete",
        &format!("https://huggingface.co/datasets/{DATASET_NAME}"),
        None,
    )?;

    Ok(())
}
