use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
pub struct LimitArgs {
    /// Stop after N records. Useful for smoke tests.
    #[arg(long)]
    pub limit: Option<u64>,
}

#[derive(Debug, Parser)]
pub struct StreamArgs {
    /// Disable streaming mode (download shards/archives before processing).
    #[arg(long)]
    pub no_stream: bool,
}

impl StreamArgs {
    pub fn streaming(&self) -> bool {
        !self.no_stream
    }
}

#[derive(Debug, Parser)]
pub struct OutputLimitArgs {
    /// Output JSONL path.
    #[arg(long)]
    pub output: PathBuf,

    #[command(flatten)]
    pub limit: LimitArgs,
}

pub fn print_export_summary(
    title: &str,
    stats: &crate::Stats,
    output: &std::path::Path,
    source: &str,
) {
    println!();
    println!("{}", "=".repeat(48));
    println!("{title}");
    println!("{}", "=".repeat(48));
    println!(
        "Total documents : {}",
        crate::stats::format_number(stats.total_docs)
    );
    println!(
        "Total characters: {}",
        crate::stats::format_number(stats.total_chars)
    );
    println!(
        "Avg doc length  : {} chars",
        crate::stats::format_float(stats.avg_len())
    );
    println!("Output file     : {}", output.display());
    println!("Source          : {source}");
    println!("{}", "=".repeat(48));
}

pub fn print_merge_summary(
    stats: &crate::Stats,
    output: &std::path::Path,
    source_status: &[(String, bool, u64)],
) {
    println!();
    println!("{}", "=".repeat(48));
    println!("Somali corpus merge complete");
    println!("{}", "=".repeat(48));
    println!(
        "Total documents : {}",
        crate::stats::format_number(stats.total_docs)
    );
    println!(
        "Total characters: {}",
        crate::stats::format_number(stats.total_chars)
    );
    println!(
        "Avg doc length  : {} chars",
        crate::stats::format_float(stats.avg_len())
    );
    println!("Output file     : {}", output.display());
    println!("Per-source counts:");
    for (source, exists, count) in source_status {
        let status = if *exists { "found" } else { "missing" };
        println!("  - {source:6}: {count:>10} docs ({status})");
    }
    println!("{}", "=".repeat(48));
}
