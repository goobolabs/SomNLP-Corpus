use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::LimitArgs;
use corpus_tools::tanzil::{self, download_tanzil};

const DEFAULT_OUTPUT: &str = "data/raw/quran-tanzil/translation.json";

#[derive(Debug, Parser)]
#[command(about = "Download the Tanzil Somali Qur'an translation (Abduh) as JSONL")]
struct Args {
    /// Output JSONL path.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    /// Tanzil translation id. Somali has only `so.abduh`.
    #[arg(long, default_value = tanzil::TRANSLATION_ID)]
    translation_id: String,

    #[command(flatten)]
    limit: LimitArgs,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let url = tanzil::download_url(&args.translation_id);
    let stats = download_tanzil(&args.output, &url, args.limit.limit)?;

    corpus_tools::cli::print_export_summary(
        "Tanzil Somali Qur'an translation export complete",
        &stats,
        &args.output,
        &tanzil::source_url(),
    );
    Ok(())
}
