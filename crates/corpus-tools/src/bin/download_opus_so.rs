use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::export::export_parquet_struct_shards;
use corpus_tools::hf::{filter_paths, HfClient};

const DATASET_NAME: &str = "Helsinki-NLP/opus_paracrawl";
const DATASET_CONFIG: &str = "en-so";
const DEFAULT_LANGUAGE: &str = "so";
const DEFAULT_OUTPUT: &str = "data/raw/opus/opus_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download Somali OPUS ParaCrawl sentences and export JSONL")]
struct Args {
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    #[command(flatten)]
    limit: LimitArgs,

    #[command(flatten)]
    stream: StreamArgs,

    #[arg(long, default_value = DEFAULT_LANGUAGE)]
    language: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let hf = HfClient::new();
    let shards = filter_paths(hf.list_files(DATASET_NAME, DATASET_CONFIG)?, ".parquet");

    export_parquet_struct_shards(
        &hf,
        DATASET_NAME,
        &shards,
        &args.output,
        "translation",
        &args.language,
        args.limit.limit,
        args.stream.streaming(),
        "OPUS Somali export complete",
        &format!("https://huggingface.co/datasets/{DATASET_NAME} (config: {DATASET_CONFIG})"),
    )?;

    Ok(())
}
