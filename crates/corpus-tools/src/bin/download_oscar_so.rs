use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::export::export_parquet_shards;
use corpus_tools::hf::{filter_paths, is_auth_error, print_oscar_auth_help, HfClient};

const DATASET_NAME: &str = "oscar-corpus/OSCAR-2301";
const DEFAULT_LANGUAGE: &str = "so";
const DEFAULT_OUTPUT: &str = "data/raw/oscar/oscar_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali OSCAR-2301 subset and export JSONL")]
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
    let shard_result = hf.list_files(DATASET_NAME, &args.language);
    let shards = match shard_result {
        Ok(paths) => filter_paths(paths, ".parquet"),
        Err(error) => {
            if is_auth_error(&error) {
                print_oscar_auth_help(&error);
                std::process::exit(1);
            }
            return Err(error);
        }
    };

    if let Err(error) = export_parquet_shards(
        &hf,
        DATASET_NAME,
        &shards,
        &args.output,
        "text",
        args.limit.limit,
        args.stream.streaming(),
        "OSCAR Somali export complete",
        &format!("https://huggingface.co/datasets/{DATASET_NAME}"),
        None,
    ) {
        if is_auth_error(&error) {
            print_oscar_auth_help(&error);
            std::process::exit(1);
        }
        return Err(error);
    }

    Ok(())
}
