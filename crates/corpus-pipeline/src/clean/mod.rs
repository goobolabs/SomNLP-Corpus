//! Clean stage: transform raw merged records into corpus records (see
//! docs/CLEANING_PLAN.md). Each sub-module is a pure, unit-tested step.

pub mod entities;
pub mod gates;
pub mod heuristics;
pub mod mojibake;
pub mod repeated;
pub mod stage;
pub mod strip;
pub mod whitespace;

pub use stage::{clean_record, clean_text, CleanResult};
