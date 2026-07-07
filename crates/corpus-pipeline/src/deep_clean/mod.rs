//! v0.2 deep-clean stage modules.

pub mod boilerplate;
pub mod config;
pub mod contact;
pub mod html;
pub mod intra_dedup;
pub mod lid;
pub mod normalize;
pub mod stage;

pub use config::{DeepCleanConfig, DeepCleanLidConfig, IntraDedupConfig};
pub use stage::{deep_clean_record, DeepCleanResult};
