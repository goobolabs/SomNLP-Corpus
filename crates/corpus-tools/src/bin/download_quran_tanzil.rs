use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use corpus_tools::cli::LimitArgs;
use corpus_tools::jsonl::write_texts;
use corpus_tools::quran;
use corpus_tools::tanzil::{self, download_tanzil};
use reqwest::Client;

const DEFAULT_OUTPUT: &str = "data/raw/quran-tanzil/translation.json";
const DEFAULT_CONCURRENCY: usize = 8;
const USER_AGENT: &str = "corpus-tools/0.1";

#[derive(Debug, Parser)]
#[command(about = "Download the Tanzil Somali Qur'an translation (Abduh) as JSONL")]
struct Args {
    /// Output JSONL path.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    /// Also write footnotes here. Tanzil publishes none, so they are fetched
    /// from QuranEnc — a different translator, whose notes are keyed to their
    /// own text and do not line up with the Abduh translation above.
    #[arg(long)]
    footnotes_output: Option<PathBuf>,

    /// Surahs to fetch concurrently when collecting footnotes.
    #[arg(long, default_value_t = DEFAULT_CONCURRENCY)]
    concurrency: usize,

    /// Tanzil translation id. Somali has only `so.abduh`.
    #[arg(long, default_value = tanzil::TRANSLATION_ID)]
    translation_id: String,

    #[command(flatten)]
    limit: LimitArgs,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let url = tanzil::download_url(&args.translation_id);
    let translation_stats = download_tanzil(&args.output, &url, args.limit.limit)?;
    corpus_tools::cli::print_export_summary(
        "Tanzil Somali Qur'an translation export complete",
        &translation_stats,
        &args.output,
        &tanzil::source_url(),
    );

    // Opt-in, and summarised under its own source line, so the split provenance
    // stays visible rather than being implied by the file sitting next door.
    if let Some(path) = &args.footnotes_output {
        let footnotes = fetch_quranenc_footnotes(args.concurrency.max(1))?;
        let footnote_stats = write_texts(path, &footnotes, args.limit.limit)?;
        corpus_tools::cli::print_export_summary(
            "Qur'an Somali footnotes export complete (QuranEnc — different translator)",
            &footnote_stats,
            path,
            &quran::source_url(),
        );
    }

    Ok(())
}

/// Collect the QuranEnc footnotes.
///
/// The Tanzil download above is blocking, so the async QuranEnc fetch gets its
/// own runtime, built only once the footnotes are actually asked for. The two
/// run in sequence, never nested.
fn fetch_quranenc_footnotes(concurrency: usize) -> Result<Vec<String>> {
    let runtime = tokio::runtime::Runtime::new().context("starting the async runtime")?;
    runtime.block_on(async {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .context("building the HTTP client")?;
        let corpus = quran::fetch_corpus(&client, concurrency).await?;
        Ok(corpus.footnotes)
    })
}
