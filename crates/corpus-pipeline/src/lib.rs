//! SomNLP-Corpus processing pipeline: clean, language identification, and
//! near-deduplication stages. See docs/CLEANING_PLAN.md.

pub mod clean;
pub mod config;
pub mod deep_clean;
pub mod drop_inspect;
pub mod io;
pub mod lid;
pub mod near_dedup;
pub mod pipeline_summary;
pub mod progress;
pub mod report;
