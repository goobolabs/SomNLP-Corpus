use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use corpus_tools::cli::LimitArgs;
use corpus_tools::nllb::{self, download_nllb, Filters, Format, Options};

const DEFAULT_OUTPUT: &str = "data/raw/nllb/nllb_so.jsonl";

#[derive(Debug, Parser)]
#[command(about = "Download the official NLLB English–Somali pair and export JSONL")]
struct Args {
    /// Output JSONL path.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    output: PathBuf,

    /// Directory for the downloaded archive (defaults to the output directory).
    #[arg(long)]
    archive_dir: Option<PathBuf>,

    #[arg(long, value_enum, default_value = "jsonl")]
    format: Format,

    #[command(flatten)]
    limit: LimitArgs,

    /// Minimum LASER alignment score.
    #[arg(long)]
    min_laser: Option<f64>,

    /// Minimum English language-ID score, within [0, 1].
    #[arg(long)]
    min_eng_lid: Option<f64>,

    /// Minimum Somali language-ID score, within [0, 1].
    #[arg(long)]
    min_som_lid: Option<f64>,

    /// Minimum Somali length in characters.
    #[arg(long)]
    min_chars: Option<usize>,

    /// Maximum character-length ratio between the two sides (>= 1).
    #[arg(long)]
    max_length_ratio: Option<f64>,

    /// Drop repeated Somali sentences.
    #[arg(long)]
    dedup: bool,

    /// Re-download even when a valid archive is already present.
    #[arg(long)]
    overwrite: bool,

    /// Delete the archive after a successful export.
    #[arg(long)]
    delete_archive: bool,

    /// Override the source URL. Must stay on the official bucket.
    #[arg(long, default_value = nllb::OFFICIAL_URL)]
    source_url: String,

    /// Disable the download progress bar.
    #[arg(long)]
    no_progress: bool,
}

impl Args {
    fn into_options(self) -> Options {
        Options {
            output: self.output,
            archive_dir: self.archive_dir,
            format: self.format,
            limit: self.limit.limit,
            filters: Filters {
                min_laser: self.min_laser,
                min_eng_lid: self.min_eng_lid,
                min_som_lid: self.min_som_lid,
                min_chars: self.min_chars,
                max_length_ratio: self.max_length_ratio,
            },
            dedup: self.dedup,
            overwrite: self.overwrite,
            delete_archive: self.delete_archive,
            source_url: self.source_url,
            progress: !self.no_progress,
        }
    }
}

fn main() -> Result<()> {
    let options = Args::parse().into_options();
    download_nllb(&options)?;
    Ok(())
}
