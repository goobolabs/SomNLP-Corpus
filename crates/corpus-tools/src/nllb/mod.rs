//! NLLB English–Somali (`allenai/nllb`, `eng_Latn-som_Latn`) toolkit.
//!
//! The raw data is one gzip-compressed, nine-column TSV served from the
//! official AllenAI bucket. This module downloads **only** that single
//! language-pair file — never the complete NLLB corpus — verifies it, and
//! stream-converts it into the requested format while optionally collecting an
//! audit report:
//!
//! ```text
//! download → parse → normalize → filter → deduplicate → write → audit
//! ```
//!
//! Filtering is opt-in: with no thresholds set, every valid non-empty row is
//! kept. Nothing is lowercased, spell-corrected or invented — missing metadata
//! stays missing.
//!
//! NLLB is machine-mined bitext. LASER and language-ID scores are useful
//! signals, not guarantees, and the Somali side needs further cleaning (see
//! the corpus-pipeline) before it is training-ready.

pub mod audit;
pub mod config;
pub mod download;
pub mod export;
pub mod filter;
pub mod normalize;
pub mod pipeline;
pub mod process;
pub mod record;

pub use audit::{Audit, AuditReport};
pub use config::{
    is_official_host, official_url, source_url, Layout, DATASET_REPO, DEFAULT_ROOT, FIELD_NAMES,
    LANGUAGE_PAIR, RAW_FILENAME, SOURCE_TAG,
};
pub use download::{download_archive, verify_gzip, DownloadOptions};
pub use export::{CorpusWriter, Format};
pub use filter::{FilterOptions, Rejection, RejectionCounts};
pub use pipeline::{run, RunOptions, RunSummary};
pub use process::{process_stream, ProcessOptions, ProcessSummary};
pub use record::{build_record, parse_raw_line, NllbRecord, RawRow, RowError};
