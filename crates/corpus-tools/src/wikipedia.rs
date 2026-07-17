//! Wikipedia download configuration and logic.

use std::path::Path;

use anyhow::Result;
use crate::export::export_parquet_shards;
use crate::hf::{filter_paths, HfClient};

pub const DATASET_NAME: &str = "wikimedia/wikipedia";
pub const DATASET_CONFIG: &str = "20231101.so";

/// Downloads the Somali Wikipedia subset from Hugging Face and exports it as JSONL.
pub fn download_wikipedia(output: &Path, limit: Option<usize>, stream: bool) -> Result<()> {
    let hf = HfClient::new();
    let shards = filter_paths(hf.list_files(DATASET_NAME, DATASET_CONFIG)?, ".parquet");

    export_parquet_shards(
        &hf,
        DATASET_NAME,
        &shards,
        output,
        "text",
        limit,
        stream,
        "Wikipedia Somali export complete",
        &format!("https://huggingface.co/datasets/{DATASET_NAME}"),
        None,
    )?;

    Ok(())
}
