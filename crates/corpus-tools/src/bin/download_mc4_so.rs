use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::export::export_json_gz_shards;
use corpus_tools::hf::HfClient;
use corpus_tools::mc4;

const DATASET_NAME: &str = "allenai/c4";
const DEFAULT_OUTPUT: &str = "data/raw/mc4/mc4_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali mC4 subset and export JSONL")]
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
    let shards = mc4::mc4_so_train_shards();

    if shards.is_empty() {
        bail!("no mC4 Somali shards configured");
    }

    export_json_gz_shards(
        &hf,
        DATASET_NAME,
        &shards,
        &args.output,
        args.limit.limit,
        args.stream.streaming(),
        "mC4 Somali export complete",
        &format!("https://huggingface.co/datasets/{DATASET_NAME} (config: so)"),
    )?;

    Ok(())
}
