//! XL-Sum Somali (`csebuetnlp/xlsum`) toolkit.
//!
//! Two entry points share one source configuration:
//!
//! * [`download_xlsum`] — the raw single-field JSONL downloader every other
//!   source in this crate also exposes (`download_xlsum_so` binary). Text is
//!   left untouched; cleaning is the corpus-pipeline's job.
//! * [`pipeline::run_pipeline`] — the staged toolkit (`xlsum_so` binary):
//!   download → inspect → extract → clean → export → stats, producing a
//!   cleaned corpus in JSONL / CSV / plain text plus schema and metadata
//!   reports under one output root.

pub mod clean;
pub mod config;
pub mod download;
pub mod export;
pub mod extract;
pub mod inspect;
pub mod pipeline;
pub mod record;
pub mod stats;

pub use config::{
    source_url, Layout, DATASET_CONFIG, DATASET_NAME, DEFAULT_ROOT, FIELDS, SOURCE_TAG, SPLITS,
};
pub use download::{download_xlsum, fetch_shards, Shard};
pub use record::XlsumRecord;
