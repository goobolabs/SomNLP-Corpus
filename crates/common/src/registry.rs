//! Static source registry: the single vocabulary for source keys, classes,
//! licenses, and per-source processing policy (see docs/SOURCES.md and
//! docs/CLEANING_PLAN.md).

use crate::types::{License, SourceKey};

/// Whether a source produces full documents or individual aligned sentences.
/// Controls length floors, LID policy, and near-dedup participation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceClass {
    Document,
    Sentence,
}

/// How language identification treats a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LidPolicy {
    /// Drop records below the configured confidence threshold.
    Gate,
    /// Record the language score but never reject (curated parallel data).
    TagOnly,
}

/// Registry metadata for one data source.
#[derive(Debug, Clone)]
pub struct SourceEntry {
    pub key: &'static str,
    pub class: SourceClass,
    pub lid_policy: LidPolicy,
    pub near_dedup: bool,
    /// SPDX-style license identifier; mapped to [`License`] via [`SourceEntry::license`].
    pub license_id: &'static str,
}

impl SourceEntry {
    pub fn source_key(&self) -> SourceKey {
        SourceKey(self.key.to_string())
    }

    /// Resolve the license identifier to a [`License`] value.
    pub fn license(&self) -> License {
        match self.license_id {
            "CC0-1.0" => License::Cc0_1_0,
            "CC-BY-4.0" => License::CcBy4_0,
            "CC-BY-SA-4.0" => License::CcBySa4_0,
            "MIT" => License::Mit,
            "Apache-2.0" => License::Apache2_0,
            "public-domain" => License::PublicDomain,
            other => License::Other(other.to_string()),
        }
    }
}

/// Track A sources, in registry order. See docs/SOURCES.md.
pub const SOURCES: &[SourceEntry] = &[
    SourceEntry {
        key: "hplt",
        class: SourceClass::Document,
        lid_policy: LidPolicy::Gate,
        near_dedup: true,
        license_id: "CC0-1.0",
    },
    SourceEntry {
        key: "cc100",
        class: SourceClass::Document,
        lid_policy: LidPolicy::Gate,
        near_dedup: true,
        license_id: "CC-BY-SA-4.0",
    },
    SourceEntry {
        key: "mc4",
        class: SourceClass::Document,
        lid_policy: LidPolicy::Gate,
        near_dedup: true,
        license_id: "ODC-BY",
    },
    SourceEntry {
        key: "madlad",
        class: SourceClass::Document,
        lid_policy: LidPolicy::Gate,
        near_dedup: true,
        license_id: "ODC-BY",
    },
    SourceEntry {
        key: "opus",
        class: SourceClass::Sentence,
        lid_policy: LidPolicy::TagOnly,
        near_dedup: false,
        license_id: "CC0-1.0",
    },
    SourceEntry {
        key: "mt560",
        class: SourceClass::Sentence,
        lid_policy: LidPolicy::TagOnly,
        near_dedup: false,
        license_id: "CC-BY-4.0",
    },
    SourceEntry {
        key: "quran",
        class: SourceClass::Sentence,
        lid_policy: LidPolicy::TagOnly,
        near_dedup: false,
        license_id: "Other",
    },
];

/// Look up a source entry by its registry key.
pub fn lookup(key: &str) -> Option<&'static SourceEntry> {
    SOURCES.iter().find(|entry| entry.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_track_a_keys_present() {
        for key in ["hplt", "cc100", "mc4", "madlad", "opus", "mt560"] {
            assert!(lookup(key).is_some(), "missing source: {key}");
        }
    }

    #[test]
    fn sentence_sources_skip_near_dedup() {
        assert!(!lookup("opus").unwrap().near_dedup);
        assert!(!lookup("mt560").unwrap().near_dedup);
        assert_eq!(lookup("opus").unwrap().class, SourceClass::Sentence);
    }

    #[test]
    fn document_sources_use_gate_policy() {
        assert_eq!(lookup("hplt").unwrap().lid_policy, LidPolicy::Gate);
        assert!(lookup("hplt").unwrap().near_dedup);
    }

    #[test]
    fn unknown_key_is_none() {
        assert!(lookup("oscar").is_none());
    }
}
