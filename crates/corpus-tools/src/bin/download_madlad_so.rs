use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::export::export_json_gz_shards;
use corpus_tools::hf::HfClient;
use corpus_tools::madlad;

const DEFAULT_OUTPUT: &str = "data/raw/madlad/madlad_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali MADLAD-400 subset and export JSONL")]
struct Args {
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    #[command(flatten)]
    limit: LimitArgs,

    #[command(flatten)]
    stream: StreamArgs,

    /// Also include the noisy MADLAD-400 split (clean is exported by default).
    #[arg(long)]
    include_noisy: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let hf = HfClient::new();
    let shards = madlad::madlad_so_shards(args.include_noisy);

    export_json_gz_shards(
        &hf,
        madlad::dataset_repo(),
        &shards,
        &args.output,
        args.limit.limit,
        args.stream.streaming(),
        "MADLAD-400 Somali export complete",
        &madlad::source_url(args.include_noisy),
    )?;

    Ok(())
}
