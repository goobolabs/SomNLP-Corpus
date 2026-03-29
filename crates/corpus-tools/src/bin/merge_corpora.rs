use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use clap::Parser;
use corpus_tools::cli;
use corpus_tools::jsonl::{is_non_empty, read_jsonl_texts, JsonlWriter};

const DEFAULT_RAW_DIR: &str = "data/raw";
const DEFAULT_OUTPUT: &str = "data/merged/merged_so.jsonl";
const SOURCES: &[&str] = &["cc100", "hplt", "mc4", "opus", "madlad", "mt560", "oscar"];

#[derive(Debug, Parser)]
#[command(about = "Merge raw Somali JSONL corpora into a single file")]
struct Args {
    #[arg(long, default_value = DEFAULT_RAW_DIR)]
    raw_dir: PathBuf,

    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    #[arg(long, num_args = 1.., default_values_t = default_sources())]
    sources: Vec<String>,

    #[arg(long)]
    limit: Option<u64>,
}

fn default_sources() -> Vec<String> {
    SOURCES.iter().map(|source| (*source).to_string()).collect()
}

fn source_path(raw_dir: &Path, source: &str) -> PathBuf {
    raw_dir.join(source).join(format!("{source}_so.jsonl"))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut writer = JsonlWriter::create(&args.output, "Merging")?;
    let mut written = 0u64;
    let mut source_status = Vec::new();

    for source in &args.sources {
        let path = source_path(&args.raw_dir, source);
        let exists = path.exists();
        let mut count = 0u64;

        if !exists {
            eprintln!("Skipping missing source: {}", path.display());
            source_status.push((source.clone(), false, 0));
            continue;
        }

        for text in read_jsonl_texts(&path)? {
            if args.limit.is_some_and(|limit| written >= limit) {
                break;
            }
            let text = text?;
            if !is_non_empty(&text) {
                continue;
            }
            writer.write_text_source(source, &text)?;
            written += 1;
            count += 1;
        }

        source_status.push((source.clone(), true, count));
        if args.limit.is_some_and(|limit| written >= limit) {
            break;
        }
    }

    if writer.stats.total_docs == 0 {
        bail!("No documents merged. Download raw sources first or check --sources.");
    }

    let stats = writer.stats.clone();
    writer.finish();
    cli::print_merge_summary(&stats, &args.output, &source_status);
    Ok(())
}
