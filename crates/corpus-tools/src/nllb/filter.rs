//! Threshold filtering and the stable set of rejection reasons.

use anyhow::{bail, Result};

use crate::nllb::record::{length_ratio, RawRow};

/// Every reason a row can be dropped, in the fixed order used by the run
/// summary and the audit report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rejection {
    MalformedRow,
    InvalidNumeric,
    EmptyEnglish,
    EmptySomali,
    LaserTooLow,
    EnglishLidTooLow,
    SomaliLidTooLow,
    EnglishTooShort,
    SomaliTooShort,
    EnglishTooLong,
    SomaliTooLong,
    LengthRatioTooHigh,
    DuplicatePair,
    DuplicateSomali,
}

impl Rejection {
    pub const ALL: [Rejection; 14] = [
        Rejection::MalformedRow,
        Rejection::InvalidNumeric,
        Rejection::EmptyEnglish,
        Rejection::EmptySomali,
        Rejection::LaserTooLow,
        Rejection::EnglishLidTooLow,
        Rejection::SomaliLidTooLow,
        Rejection::EnglishTooShort,
        Rejection::SomaliTooShort,
        Rejection::EnglishTooLong,
        Rejection::SomaliTooLong,
        Rejection::LengthRatioTooHigh,
        Rejection::DuplicatePair,
        Rejection::DuplicateSomali,
    ];

    /// Report label; also the JSON key in `rejected_rows_by_reason`.
    pub fn label(self) -> &'static str {
        match self {
            Rejection::MalformedRow => "malformed row",
            Rejection::InvalidNumeric => "invalid numeric value",
            Rejection::EmptyEnglish => "empty English",
            Rejection::EmptySomali => "empty Somali",
            Rejection::LaserTooLow => "LASER score too low",
            Rejection::EnglishLidTooLow => "English LID too low",
            Rejection::SomaliLidTooLow => "Somali LID too low",
            Rejection::EnglishTooShort => "English too short",
            Rejection::SomaliTooShort => "Somali too short",
            Rejection::EnglishTooLong => "English too long",
            Rejection::SomaliTooLong => "Somali too long",
            Rejection::LengthRatioTooHigh => "length ratio too high",
            Rejection::DuplicatePair => "duplicate pair",
            Rejection::DuplicateSomali => "duplicate Somali sentence",
        }
    }

    fn index(self) -> usize {
        Rejection::ALL
            .iter()
            .position(|reason| *reason == self)
            .expect("every rejection is listed in ALL")
    }
}

/// Rejection tallies, kept in [`Rejection::ALL`] order.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RejectionCounts([u64; 14]);

impl RejectionCounts {
    pub fn record(&mut self, reason: Rejection) {
        self.0[reason.index()] += 1;
    }

    pub fn get(&self, reason: Rejection) -> u64 {
        self.0[reason.index()]
    }

    pub fn total(&self) -> u64 {
        self.0.iter().sum()
    }

    /// Every reason with its count, in report order.
    pub fn iter(&self) -> impl Iterator<Item = (Rejection, u64)> + '_ {
        Rejection::ALL
            .into_iter()
            .map(move |reason| (reason, self.get(reason)))
    }
}

/// Quality thresholds. Every field defaults to `None`, which keeps all valid
/// non-empty rows — filtering is opt-in, never silently applied.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FilterOptions {
    pub min_laser: Option<f64>,
    pub min_eng_lid: Option<f64>,
    pub min_som_lid: Option<f64>,
    pub min_eng_chars: Option<usize>,
    pub min_som_chars: Option<usize>,
    pub max_eng_chars: Option<usize>,
    pub max_som_chars: Option<usize>,
    pub max_length_ratio: Option<f64>,
}

impl FilterOptions {
    /// Reject thresholds that can never be satisfied, before anything is
    /// downloaded.
    pub fn validate(&self) -> Result<()> {
        if let (Some(min), Some(max)) = (self.min_eng_chars, self.max_eng_chars) {
            if min > max {
                bail!("--min-eng-chars cannot exceed --max-eng-chars");
            }
        }
        if let (Some(min), Some(max)) = (self.min_som_chars, self.max_som_chars) {
            if min > max {
                bail!("--min-som-chars cannot exceed --max-som-chars");
            }
        }
        if self.max_length_ratio.is_some_and(|ratio| ratio < 1.0) {
            bail!("--max-length-ratio must be >= 1.0");
        }
        for (flag, value) in [
            ("--min-eng-lid", self.min_eng_lid),
            ("--min-som-lid", self.min_som_lid),
        ] {
            if value.is_some_and(|score| !(0.0..=1.0).contains(&score)) {
                bail!("{flag} must be within [0, 1]");
            }
        }
        Ok(())
    }

    /// The first threshold `row` fails, or `None` when it is kept.
    ///
    /// Empty English or Somali text is always rejected, whatever the
    /// thresholds say — a pair with one side missing is not a pair.
    pub fn check(&self, row: &RawRow) -> Option<Rejection> {
        if row.english.is_empty() {
            return Some(Rejection::EmptyEnglish);
        }
        if row.somali.is_empty() {
            return Some(Rejection::EmptySomali);
        }
        if self
            .min_laser
            .is_some_and(|min| row.laser_score < min)
        {
            return Some(Rejection::LaserTooLow);
        }
        if self
            .min_eng_lid
            .is_some_and(|min| row.english_lid_score < min)
        {
            return Some(Rejection::EnglishLidTooLow);
        }
        if self
            .min_som_lid
            .is_some_and(|min| row.somali_lid_score < min)
        {
            return Some(Rejection::SomaliLidTooLow);
        }

        let english = row.english.chars().count();
        let somali = row.somali.chars().count();
        if self.min_eng_chars.is_some_and(|min| english < min) {
            return Some(Rejection::EnglishTooShort);
        }
        if self.min_som_chars.is_some_and(|min| somali < min) {
            return Some(Rejection::SomaliTooShort);
        }
        if self.max_eng_chars.is_some_and(|max| english > max) {
            return Some(Rejection::EnglishTooLong);
        }
        if self.max_som_chars.is_some_and(|max| somali > max) {
            return Some(Rejection::SomaliTooLong);
        }
        if self
            .max_length_ratio
            .is_some_and(|max| length_ratio(&row.english, &row.somali) > max)
        {
            return Some(Rejection::LengthRatioTooHigh);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(english: &str, somali: &str) -> RawRow {
        RawRow {
            english: english.to_string(),
            somali: somali.to_string(),
            laser_score: 1.2,
            english_lid_score: 0.99,
            somali_lid_score: 0.98,
            english_source: "src-en".into(),
            english_url: None,
            somali_source: "src-so".into(),
            somali_url: None,
        }
    }

    #[test]
    fn defaults_keep_every_non_empty_row() {
        let mut low = row("hello", "salaan");
        low.laser_score = -5.0;
        low.english_lid_score = 0.01;
        low.somali_lid_score = 0.01;
        assert_eq!(FilterOptions::default().check(&low), None);
    }

    #[test]
    fn empty_sides_are_always_rejected() {
        let options = FilterOptions::default();
        assert_eq!(options.check(&row("", "salaan")), Some(Rejection::EmptyEnglish));
        assert_eq!(options.check(&row("hello", "")), Some(Rejection::EmptySomali));
    }

    #[test]
    fn laser_and_lid_thresholds_apply() {
        let options = FilterOptions {
            min_laser: Some(1.0),
            min_eng_lid: Some(0.9),
            min_som_lid: Some(0.9),
            ..FilterOptions::default()
        };

        let mut low_laser = row("hello", "salaan");
        low_laser.laser_score = 0.5;
        assert_eq!(options.check(&low_laser), Some(Rejection::LaserTooLow));

        let mut low_eng = row("hello", "salaan");
        low_eng.english_lid_score = 0.5;
        assert_eq!(options.check(&low_eng), Some(Rejection::EnglishLidTooLow));

        let mut low_som = row("hello", "salaan");
        low_som.somali_lid_score = 0.5;
        assert_eq!(options.check(&low_som), Some(Rejection::SomaliLidTooLow));

        assert_eq!(options.check(&row("hello", "salaan")), None);
    }

    #[test]
    fn character_bounds_count_characters() {
        let options = FilterOptions {
            min_eng_chars: Some(5),
            max_som_chars: Some(4),
            ..FilterOptions::default()
        };
        assert_eq!(options.check(&row("hi", "koob")), Some(Rejection::EnglishTooShort));
        assert_eq!(
            options.check(&row("long enough", "aad u dheer")),
            Some(Rejection::SomaliTooLong)
        );
        // Four two-byte characters are within a four-character bound.
        assert_eq!(options.check(&row("hello", "éééé")), None);
    }

    #[test]
    fn length_ratio_threshold_applies() {
        let options = FilterOptions {
            max_length_ratio: Some(3.0),
            ..FilterOptions::default()
        };
        assert_eq!(
            options.check(&row(&"a".repeat(100), "b")),
            Some(Rejection::LengthRatioTooHigh)
        );
    }

    #[test]
    fn impossible_thresholds_are_rejected_upfront() {
        assert!(FilterOptions {
            min_eng_chars: Some(100),
            max_eng_chars: Some(10),
            ..FilterOptions::default()
        }
        .validate()
        .is_err());

        assert!(FilterOptions {
            max_length_ratio: Some(0.5),
            ..FilterOptions::default()
        }
        .validate()
        .is_err());

        assert!(FilterOptions {
            min_som_lid: Some(1.5),
            ..FilterOptions::default()
        }
        .validate()
        .is_err());

        assert!(FilterOptions::default().validate().is_ok());
    }

    #[test]
    fn counts_stay_in_report_order() {
        let mut counts = RejectionCounts::default();
        counts.record(Rejection::DuplicatePair);
        counts.record(Rejection::EmptySomali);
        counts.record(Rejection::EmptySomali);

        assert_eq!(counts.get(Rejection::EmptySomali), 2);
        assert_eq!(counts.total(), 3);

        let labels: Vec<&str> = counts.iter().map(|(reason, _)| reason.label()).collect();
        assert_eq!(labels[0], "malformed row");
        assert_eq!(labels[13], "duplicate Somali sentence");
    }
}
