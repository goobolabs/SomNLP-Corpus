use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::LimitArgs;
use corpus_tools::jsonl::{is_non_empty, JsonlWriter};
use corpus_tools::quran;
use corpus_tools::Stats;
use reqwest::Client;

const DEFAULT_TRANSLATION_OUTPUT: &str = "data/raw/quran/translation.json";
const DEFAULT_FOOTNOTES_OUTPUT: &str = "data/raw/quran/footnotes.json";
const DEFAULT_CONCURRENCY: usize = 8;

#[derive(Debug, Parser)]
#[command(about = "Download the Somali Qur'an (Yacob Yusuf) translation and footnotes as JSONL")]
struct Args {
    #[arg(long, default_value = DEFAULT_TRANSLATION_OUTPUT)]
    translation_output: PathBuf,

    #[arg(long, default_value = DEFAULT_FOOTNOTES_OUTPUT)]
    footnotes_output: PathBuf,

    /// Number of surahs to fetch concurrently.
    #[arg(long, default_value_t = DEFAULT_CONCURRENCY)]
    concurrency: usize,

    #[command(flatten)]
    limit: LimitArgs,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let client = Client::builder().user_agent("corpus-tools/0.1").build()?;
    let corpus = quran::fetch_corpus(&client, args.concurrency.max(1)).await?;

    let translation_stats =
        write_dataset(&args.translation_output, &corpus.translations, args.limit.limit)?;
    let footnotes_stats =
        write_dataset(&args.footnotes_output, &corpus.footnotes, args.limit.limit)?;

    let source = quran::source_url();
    corpus_tools::cli::print_export_summary(
        "Qur'an Somali translation export complete",
        &translation_stats,
        &args.translation_output,
        &source,
    );
    corpus_tools::cli::print_export_summary(
        "Qur'an Somali footnotes export complete",
        &footnotes_stats,
        &args.footnotes_output,
        &source,
    );
    Ok(())
}

fn write_dataset(output: &Path, texts: &[String], limit: Option<u64>) -> Result<Stats> {
    let mut writer = JsonlWriter::create(output, "Writing")?;
    let mut written = 0u64;
    for text in texts {
        if limit.is_some_and(|limit| written >= limit) {
            break;
        }
        if !is_non_empty(text) {
            continue;
        }
        writer.write_text(text)?;
        written += 1;
    }
    let stats = writer.stats.clone();
    writer.finish();
    Ok(stats)
}
