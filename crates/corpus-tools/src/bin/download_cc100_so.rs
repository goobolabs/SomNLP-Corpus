use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cc100;
use corpus_tools::cli::{LimitArgs, StreamArgs};
use corpus_tools::jsonl::{is_non_empty, JsonlWriter};
use reqwest::blocking::Client;

const DEFAULT_URL: &str = "https://data.statmt.org/cc-100/so.txt.xz";
const DEFAULT_OUTPUT: &str = "data/raw/cc100/cc100_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the Somali CC-100 corpus and export JSONL")]
struct Args {
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    #[command(flatten)]
    limit: LimitArgs,

    #[command(flatten)]
    stream: StreamArgs,

    #[arg(long, default_value = DEFAULT_URL)]
    url: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let client = Client::builder().user_agent("corpus-tools/0.1").build()?;
    let mut writer = JsonlWriter::create(&args.output, "Writing")?;
    let mut written = 0u64;

    for text in cc100::iter_documents_from_url(&client, &args.url, args.stream.streaming())? {
        if args.limit.limit.is_some_and(|limit| written >= limit) {
            break;
        }
        let text = text?;
        if !is_non_empty(&text) {
            continue;
        }
        writer.write_text(&text)?;
        written += 1;
    }

    let stats = writer.stats.clone();
    writer.finish();
    corpus_tools::cli::print_export_summary(
        "CC100 Somali export complete",
        &stats,
        &args.output,
        &args.url,
    );
    Ok(())
}
