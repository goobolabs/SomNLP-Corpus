//! Audit accumulation: what was seen, what was dropped, and what the kept
//! pairs look like.
//!
//! The audit is opt-in (`--audit`) because it keeps per-row values in memory
//! to compute medians and duplicate counts. It never changes what is exported;
//! it only describes it.

use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::fmt;

use anyhow::{Context, Result};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use regex::Regex;
use reqwest::Url;
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::nllb::config::{
    official_url, AUDIT_WARNINGS, DATASET_REPO, LANGUAGE_PAIR, RAW_FILENAME,
};
use crate::nllb::filter::{Rejection, RejectionCounts};
use crate::nllb::normalize::{pair_digest, text_digest};
use crate::nllb::record::{length_ratio, RawRow};

/// Seed for the reservoir sampler, so two runs over the same data pick the
/// same rows.
pub const SAMPLE_SEED: u64 = 42;

/// Sample rows kept per group, and normalization examples recorded.
const GROUP_SIZE: usize = 10;
const MAX_NORMALIZATION_EXAMPLES: usize = 15;
const TOP_DISTRIBUTION_ENTRIES: usize = 25;
const TOP_FREQUENT_SENTENCES: usize = 10;

/// A JSON object whose keys keep the order they were inserted in — report
/// order for rejections, count order for distributions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CountMap(Vec<(String, u64)>);

impl CountMap {
    pub fn from_pairs(pairs: Vec<(String, u64)>) -> Self {
        Self(pairs)
    }

    pub fn get(&self, key: &str) -> Option<u64> {
        self.0
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, count)| *count)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, u64)> + '_ {
        self.0.iter().map(|(name, count)| (name.as_str(), *count))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn total(&self) -> u64 {
        self.0.iter().map(|(_, count)| count).sum()
    }
}

impl Serialize for CountMap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, count) in &self.0 {
            map.serialize_entry(key, count)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for CountMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor;

        impl<'de> Visitor<'de> for OrderedVisitor {
            type Value = CountMap;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map of counts")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut access: M) -> Result<CountMap, M::Error> {
                let mut pairs = Vec::with_capacity(access.size_hint().unwrap_or(0));
                while let Some((key, count)) = access.next_entry::<String, u64>()? {
                    pairs.push((key, count));
                }
                Ok(CountMap(pairs))
            }
        }

        deserializer.deserialize_map(OrderedVisitor)
    }
}

/// Summary statistics of one numeric column; every field is `null` when no
/// value was collected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Describe {
    pub count: u64,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub stdev: Option<f64>,
}

/// Describe `values`, sorting them in place to take the median.
pub fn describe(values: &mut [f64]) -> Describe {
    if values.is_empty() {
        return Describe::default();
    }
    values.sort_by(f64::total_cmp);

    let count = values.len();
    let sum: f64 = values.iter().sum();
    let mean = sum / count as f64;
    let median = if count % 2 == 1 {
        values[count / 2]
    } else {
        (values[count / 2 - 1] + values[count / 2]) / 2.0
    };
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / count as f64;

    Describe {
        count: count as u64,
        min: Some(values[0]),
        max: Some(values[count - 1]),
        mean: Some(mean),
        median: Some(median),
        stdev: Some(if count > 1 { variance.sqrt() } else { 0.0 }),
    }
}

/// One sampled pair, carrying the exported schema minus the id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleRecord {
    pub english: String,
    pub somali: String,
    pub laser_score: f64,
    pub english_lid_score: f64,
    pub somali_lid_score: f64,
    pub english_source: String,
    pub english_url: Option<String>,
    pub somali_source: String,
    pub somali_url: Option<String>,
    pub language_pair: String,
    /// Which group the row was sampled into; set when the samples are written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_group: Option<String>,
}

impl SampleRecord {
    fn from_row(row: &RawRow) -> Self {
        Self {
            english: row.english.clone(),
            somali: row.somali.clone(),
            laser_score: row.laser_score,
            english_lid_score: row.english_lid_score,
            somali_lid_score: row.somali_lid_score,
            english_source: row.english_source.clone(),
            english_url: row.english_url.clone(),
            somali_source: row.somali_source.clone(),
            somali_url: row.somali_url.clone(),
            language_pair: LANGUAGE_PAIR.to_string(),
            sample_group: None,
        }
    }
}

/// A sample ordered by LASER score, with the arrival counter breaking ties so
/// the heaps stay deterministic.
#[derive(Debug, Clone)]
struct Ranked {
    laser: f64,
    counter: u64,
    record: SampleRecord,
}

impl PartialEq for Ranked {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Ranked {}

impl Ord for Ranked {
    fn cmp(&self, other: &Self) -> Ordering {
        self.laser
            .total_cmp(&other.laser)
            .then(self.counter.cmp(&other.counter))
    }
}

impl PartialOrd for Ranked {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Collects five deterministic sample groups over the accepted pairs.
pub struct Sampler {
    sample_size: usize,
    rng: ChaCha8Rng,
    first: Vec<SampleRecord>,
    last: VecDeque<SampleRecord>,
    /// Max-heap: the largest LASER score is evicted, leaving the lowest ten.
    lowest: BinaryHeap<Ranked>,
    /// Min-heap: the smallest LASER score is evicted, leaving the highest ten.
    highest: BinaryHeap<Reverse<Ranked>>,
    reservoir: Vec<(u64, SampleRecord)>,
    seen: u64,
    counter: u64,
}

impl Sampler {
    pub fn new(sample_size: usize) -> Self {
        Self {
            sample_size,
            rng: ChaCha8Rng::seed_from_u64(SAMPLE_SEED),
            first: Vec::with_capacity(GROUP_SIZE),
            last: VecDeque::with_capacity(GROUP_SIZE),
            lowest: BinaryHeap::new(),
            highest: BinaryHeap::new(),
            reservoir: Vec::new(),
            seen: 0,
            counter: 0,
        }
    }

    pub fn add(&mut self, row: &RawRow) {
        let record = SampleRecord::from_row(row);

        if self.first.len() < GROUP_SIZE {
            self.first.push(record.clone());
        }
        if self.last.len() == GROUP_SIZE {
            self.last.pop_front();
        }
        self.last.push_back(record.clone());

        let ranked = Ranked {
            laser: row.laser_score,
            counter: self.counter,
            record: record.clone(),
        };
        self.counter += 1;

        self.lowest.push(ranked.clone());
        if self.lowest.len() > GROUP_SIZE {
            self.lowest.pop();
        }
        self.highest.push(Reverse(ranked));
        if self.highest.len() > GROUP_SIZE {
            self.highest.pop();
        }

        if self.sample_size > 0 {
            self.seen += 1;
            if self.reservoir.len() < self.sample_size {
                self.reservoir.push((self.seen, record));
            } else {
                let slot = self.rng.gen_range(0..self.seen);
                if (slot as usize) < self.sample_size {
                    self.reservoir[slot as usize] = (self.seen, record);
                }
            }
        }
    }

    /// The five groups, each already in its display order.
    pub fn groups(&self) -> Vec<(&'static str, Vec<SampleRecord>)> {
        let mut lowest: Vec<&Ranked> = self.lowest.iter().collect();
        lowest.sort();

        let mut highest: Vec<&Ranked> = self.highest.iter().map(|entry| &entry.0).collect();
        highest.sort_by(|left, right| right.cmp(left));

        let mut random: Vec<&(u64, SampleRecord)> = self.reservoir.iter().collect();
        random.sort_by_key(|(seen, _)| *seen);

        vec![
            ("first_10_valid", self.first.clone()),
            ("last_10_valid", self.last.iter().cloned().collect()),
            (
                "lowest_10_laser",
                lowest.into_iter().map(|entry| entry.record.clone()).collect(),
            ),
            (
                "highest_10_laser",
                highest.into_iter().map(|entry| entry.record.clone()).collect(),
            ),
            (
                "random_sample",
                random.into_iter().map(|(_, record)| record.clone()).collect(),
            ),
        ]
    }

    /// Every sampled row exactly once, tagged with the group it came from.
    pub fn records(&self) -> Vec<SampleRecord> {
        let mut seen: HashSet<(String, String, u64)> = HashSet::new();
        let mut out = Vec::new();

        for (group, records) in self.groups() {
            for record in records {
                let key = (
                    record.english.clone(),
                    record.somali.clone(),
                    record.laser_score.to_bits(),
                );
                if !seen.insert(key) {
                    continue;
                }
                out.push(SampleRecord {
                    sample_group: Some(group.to_string()),
                    ..record
                });
            }
        }
        out
    }
}

/// Counts of distinct values, remembering first-seen order for stable ties.
#[derive(Debug, Default)]
struct Counter {
    entries: HashMap<String, (u64, u64)>,
    next: u64,
}

impl Counter {
    fn add(&mut self, key: &str) {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.0 += 1;
            return;
        }
        let order = self.next;
        self.next += 1;
        self.entries.insert(key.to_string(), (1, order));
    }

    /// The `limit` most common values, ties broken by first appearance.
    fn top(&self, limit: usize) -> Vec<(String, u64)> {
        let mut entries: Vec<(&String, &(u64, u64))> = self.entries.iter().collect();
        entries.sort_by(|(_, left), (_, right)| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
        entries
            .into_iter()
            .take(limit)
            .map(|(key, (count, _))| (key.clone(), *count))
            .collect()
    }
}

/// A frequently repeated sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrequentText {
    pub text: String,
    pub count: u64,
}

/// A pair whose text changed under `--normalize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizationExample {
    pub original_english: String,
    pub normalized_english: String,
    pub original_somali: String,
    pub normalized_somali: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideCounts {
    pub english: u64,
    pub somali: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenCounts {
    pub english_total: u64,
    pub somali_total: u64,
}

/// The full audit report, mirroring `audit_report.json` field for field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditReport {
    pub dataset_repository: String,
    pub language_pair: String,
    pub official_source_url: String,
    pub raw_filename: String,
    pub raw_compressed_size_bytes: Option<u64>,
    pub total_lines_inspected: u64,
    pub valid_parsed_rows: u64,
    pub accepted_rows: u64,
    pub rejected_rows_by_reason: CountMap,
    pub rejected_rows_total: u64,
    pub exact_duplicate_pair_count: u64,
    pub duplicate_english_count: u64,
    pub duplicate_somali_count: u64,
    pub duplicate_somali_different_english_count: u64,
    pub unique_english_count: u64,
    pub unique_somali_count: u64,
    pub laser_score_stats: Describe,
    pub english_lid_score_stats: Describe,
    pub somali_lid_score_stats: Describe,
    pub english_length_stats: Describe,
    pub somali_length_stats: Describe,
    pub length_ratio_stats: Describe,
    pub whitespace_token_counts: TokenCounts,
    pub english_source_distribution: CountMap,
    pub somali_source_distribution: CountMap,
    pub english_url_domain_distribution: CountMap,
    pub somali_url_domain_distribution: CountMap,
    pub missing_url_counts: SideCounts,
    pub empty_text_counts: SideCounts,
    pub identical_english_somali_count: u64,
    pub numeric_only_pair_count: u64,
    pub url_heavy_pair_count: u64,
    pub html_xml_remnant_count: u64,
    pub replacement_character_count: u64,
    pub suspicious_control_character_count: u64,
    pub repeated_character_count: u64,
    pub most_frequent_english_sentences: Vec<FrequentText>,
    pub most_frequent_somali_sentences: Vec<FrequentText>,
    pub malformed_rows: u64,
    pub invalid_numeric_rows: u64,
    pub normalization_change_examples: Vec<NormalizationExample>,
    pub warnings: Vec<String>,
}

/// Regexes for the text-quality flags.
struct QualityPatterns {
    html: Regex,
    url: Regex,
    control: Regex,
}

impl QualityPatterns {
    fn new() -> Result<Self> {
        Ok(Self {
            html: Regex::new(r"<[^>]+>|&[a-zA-Z#0-9]+;").context("compiling HTML pattern")?,
            url: Regex::new(r"https?://|www\.").context("compiling URL pattern")?,
            control: Regex::new(r"[\x00-\x08\x0b\x0c\x0e-\x1f]")
                .context("compiling control-character pattern")?,
        })
    }
}

/// Accumulates the audit over every valid parsed row.
pub struct Audit {
    patterns: QualityPatterns,
    sampler: Sampler,

    total_lines: u64,
    malformed: u64,
    invalid_numeric: u64,
    valid_parsed: u64,
    accepted: u64,
    rejections: RejectionCounts,

    pair_hashes: HashSet<[u8; 16]>,
    english_hashes: HashSet<[u8; 16]>,
    somali_hashes: HashSet<[u8; 16]>,
    english_non_empty: u64,
    somali_non_empty: u64,
    duplicate_pairs: u64,
    somali_to_english: HashMap<[u8; 16], [u8; 16]>,
    duplicate_somali_different_english: u64,

    laser: Vec<f64>,
    english_lid: Vec<f64>,
    somali_lid: Vec<f64>,
    english_length: Vec<f64>,
    somali_length: Vec<f64>,
    ratio: Vec<f64>,

    english_tokens: u64,
    somali_tokens: u64,

    english_source: Counter,
    somali_source: Counter,
    english_domain: Counter,
    somali_domain: Counter,
    missing_english_url: u64,
    missing_somali_url: u64,

    empty_english: u64,
    empty_somali: u64,
    identical: u64,
    numeric_only: u64,
    url_heavy: u64,
    html_remnant: u64,
    replacement_char: u64,
    control_char: u64,
    repeated_char: u64,

    english_frequency: Counter,
    somali_frequency: Counter,

    normalization_examples: Vec<NormalizationExample>,
}

impl Audit {
    pub fn new(sample_size: usize) -> Result<Self> {
        Ok(Self {
            patterns: QualityPatterns::new()?,
            sampler: Sampler::new(sample_size),
            total_lines: 0,
            malformed: 0,
            invalid_numeric: 0,
            valid_parsed: 0,
            accepted: 0,
            rejections: RejectionCounts::default(),
            pair_hashes: HashSet::new(),
            english_hashes: HashSet::new(),
            somali_hashes: HashSet::new(),
            english_non_empty: 0,
            somali_non_empty: 0,
            duplicate_pairs: 0,
            somali_to_english: HashMap::new(),
            duplicate_somali_different_english: 0,
            laser: Vec::new(),
            english_lid: Vec::new(),
            somali_lid: Vec::new(),
            english_length: Vec::new(),
            somali_length: Vec::new(),
            ratio: Vec::new(),
            english_tokens: 0,
            somali_tokens: 0,
            english_source: Counter::default(),
            somali_source: Counter::default(),
            english_domain: Counter::default(),
            somali_domain: Counter::default(),
            missing_english_url: 0,
            missing_somali_url: 0,
            empty_english: 0,
            empty_somali: 0,
            identical: 0,
            numeric_only: 0,
            url_heavy: 0,
            html_remnant: 0,
            replacement_char: 0,
            control_char: 0,
            repeated_char: 0,
            english_frequency: Counter::default(),
            somali_frequency: Counter::default(),
            normalization_examples: Vec::new(),
        })
    }

    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }

    pub fn accepted(&self) -> u64 {
        self.accepted
    }

    pub fn record_accepted(&mut self) {
        self.accepted += 1;
    }

    pub fn record_rejection(&mut self, reason: Rejection) {
        self.rejections.record(reason);
    }

    /// Carry the stream's line counters over once processing is done.
    pub fn record_totals(&mut self, total_lines: u64, malformed: u64, invalid_numeric: u64) {
        self.total_lines = total_lines;
        self.malformed = malformed;
        self.invalid_numeric = invalid_numeric;
    }

    /// Account for one successfully parsed row.
    ///
    /// `original_*` are the pre-normalization values, so the report can show
    /// what `--normalize` actually changed.
    pub fn add_valid(&mut self, row: &RawRow, original_english: &str, original_somali: &str) {
        self.valid_parsed += 1;

        self.english_source.add(non_empty_or_none(&row.english_source));
        self.somali_source.add(non_empty_or_none(&row.somali_source));
        match &row.english_url {
            None => self.missing_english_url += 1,
            Some(url) => self.english_domain.add(&domain(url)),
        }
        match &row.somali_url {
            None => self.missing_somali_url += 1,
            Some(url) => self.somali_domain.add(&domain(url)),
        }

        if row.english.is_empty() {
            self.empty_english += 1;
        }
        if row.somali.is_empty() {
            self.empty_somali += 1;
        }

        if !row.english.is_empty() && !row.somali.is_empty() {
            self.add_pair(row);
        }

        if !row.english.is_empty() {
            self.english_non_empty += 1;
            self.english_hashes.insert(text_digest(&row.english));
        }
        if !row.somali.is_empty() {
            self.somali_non_empty += 1;
            let somali = text_digest(&row.somali);
            self.somali_hashes.insert(somali);

            let english = text_digest(&row.english);
            match self.somali_to_english.get(&somali) {
                None => {
                    self.somali_to_english.insert(somali, english);
                }
                Some(previous) if *previous != english => {
                    self.duplicate_somali_different_english += 1;
                }
                Some(_) => {}
            }
        }

        if self.normalization_examples.len() < MAX_NORMALIZATION_EXAMPLES
            && (original_english != row.english || original_somali != row.somali)
        {
            self.normalization_examples.push(NormalizationExample {
                original_english: original_english.to_string(),
                normalized_english: row.english.clone(),
                original_somali: original_somali.to_string(),
                normalized_somali: row.somali.clone(),
            });
        }
    }

    /// Statistics and quality flags over a row with both sides present.
    fn add_pair(&mut self, row: &RawRow) {
        let hash = pair_digest(&row.english, &row.somali);
        if !self.pair_hashes.insert(hash) {
            self.duplicate_pairs += 1;
        }

        self.laser.push(row.laser_score);
        self.english_lid.push(row.english_lid_score);
        self.somali_lid.push(row.somali_lid_score);
        self.english_length
            .push(row.english.chars().count() as f64);
        self.somali_length.push(row.somali.chars().count() as f64);
        self.ratio.push(length_ratio(&row.english, &row.somali));
        self.english_tokens += row.english.split_whitespace().count() as u64;
        self.somali_tokens += row.somali.split_whitespace().count() as u64;

        if row.english == row.somali {
            self.identical += 1;
        }
        if !has_letter(&row.english) && !has_letter(&row.somali) {
            self.numeric_only += 1;
        }
        if self.patterns.url.is_match(&row.english) || self.patterns.url.is_match(&row.somali) {
            self.url_heavy += 1;
        }
        if self.patterns.html.is_match(&row.english) || self.patterns.html.is_match(&row.somali) {
            self.html_remnant += 1;
        }
        if row.english.contains('\u{FFFD}') || row.somali.contains('\u{FFFD}') {
            self.replacement_char += 1;
        }
        if self.patterns.control.is_match(&row.english)
            || self.patterns.control.is_match(&row.somali)
        {
            self.control_char += 1;
        }
        if has_repeated_run(&row.english) || has_repeated_run(&row.somali) {
            self.repeated_char += 1;
        }

        self.english_frequency.add(&row.english);
        self.somali_frequency.add(&row.somali);
        self.sampler.add(row);
    }

    /// Build the report. Sorts the collected values in place to take medians.
    pub fn to_report(&mut self, raw_compressed_size_bytes: Option<u64>) -> AuditReport {
        let rejections: Vec<(String, u64)> = self
            .rejections
            .iter()
            .map(|(reason, count)| (reason.label().to_string(), count))
            .collect();
        let rejected_rows_by_reason = CountMap::from_pairs(rejections);

        AuditReport {
            dataset_repository: DATASET_REPO.to_string(),
            language_pair: LANGUAGE_PAIR.to_string(),
            official_source_url: official_url(),
            raw_filename: RAW_FILENAME.to_string(),
            raw_compressed_size_bytes,
            total_lines_inspected: self.total_lines,
            valid_parsed_rows: self.valid_parsed,
            accepted_rows: self.accepted,
            rejected_rows_total: rejected_rows_by_reason.total(),
            rejected_rows_by_reason,
            exact_duplicate_pair_count: self.duplicate_pairs,
            duplicate_english_count: self.english_non_empty - self.english_hashes.len() as u64,
            duplicate_somali_count: self.somali_non_empty - self.somali_hashes.len() as u64,
            duplicate_somali_different_english_count: self.duplicate_somali_different_english,
            unique_english_count: self.english_hashes.len() as u64,
            unique_somali_count: self.somali_hashes.len() as u64,
            laser_score_stats: describe(&mut self.laser),
            english_lid_score_stats: describe(&mut self.english_lid),
            somali_lid_score_stats: describe(&mut self.somali_lid),
            english_length_stats: describe(&mut self.english_length),
            somali_length_stats: describe(&mut self.somali_length),
            length_ratio_stats: describe(&mut self.ratio),
            whitespace_token_counts: TokenCounts {
                english_total: self.english_tokens,
                somali_total: self.somali_tokens,
            },
            english_source_distribution: top_map(&self.english_source),
            somali_source_distribution: top_map(&self.somali_source),
            english_url_domain_distribution: top_map(&self.english_domain),
            somali_url_domain_distribution: top_map(&self.somali_domain),
            missing_url_counts: SideCounts {
                english: self.missing_english_url,
                somali: self.missing_somali_url,
            },
            empty_text_counts: SideCounts {
                english: self.empty_english,
                somali: self.empty_somali,
            },
            identical_english_somali_count: self.identical,
            numeric_only_pair_count: self.numeric_only,
            url_heavy_pair_count: self.url_heavy,
            html_xml_remnant_count: self.html_remnant,
            replacement_character_count: self.replacement_char,
            suspicious_control_character_count: self.control_char,
            repeated_character_count: self.repeated_char,
            most_frequent_english_sentences: frequent(&self.english_frequency),
            most_frequent_somali_sentences: frequent(&self.somali_frequency),
            malformed_rows: self.malformed,
            invalid_numeric_rows: self.invalid_numeric,
            normalization_change_examples: self.normalization_examples.clone(),
            warnings: AUDIT_WARNINGS.iter().map(|text| text.to_string()).collect(),
        }
    }
}

fn top_map(counter: &Counter) -> CountMap {
    CountMap::from_pairs(counter.top(TOP_DISTRIBUTION_ENTRIES))
}

fn frequent(counter: &Counter) -> Vec<FrequentText> {
    counter
        .top(TOP_FREQUENT_SENTENCES)
        .into_iter()
        .map(|(text, count)| FrequentText { text, count })
        .collect()
}

fn non_empty_or_none(value: &str) -> &str {
    if value.is_empty() {
        "<none>"
    } else {
        value
    }
}

/// Host (with port) of a URL, or `<unparsed>` when it is not a URL at all.
fn domain(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return "<unparsed>".to_string();
    };
    let Some(host) = parsed.host_str() else {
        return "<unparsed>".to_string();
    };
    match parsed.port() {
        Some(port) => format!("{}:{port}", host.to_ascii_lowercase()),
        None => host.to_ascii_lowercase(),
    }
}

fn has_letter(text: &str) -> bool {
    text.chars().any(char::is_alphabetic)
}

/// Whether any character repeats ten or more times in a row — a common sign of
/// scraped padding or decoration.
fn has_repeated_run(text: &str) -> bool {
    let mut previous = None;
    let mut run = 0usize;
    for ch in text.chars() {
        if Some(ch) == previous && ch != '\n' {
            run += 1;
            if run >= 10 {
                return true;
            }
        } else {
            previous = Some(ch);
            run = 1;
        }
    }
    false
}

/// Render the report as Markdown.
pub fn render_markdown(report: &AuditReport) -> String {
    let mut lines = vec![
        "# NLLB English-Somali audit report".to_string(),
        String::new(),
        format!("- Dataset repository: `{}`", report.dataset_repository),
        format!("- Language pair: `{}`", report.language_pair),
        format!("- Official source URL: {}", report.official_source_url),
        format!("- Raw filename: `{}`", report.raw_filename),
        format!(
            "- Raw compressed size (bytes): {}",
            optional(report.raw_compressed_size_bytes)
        ),
        String::new(),
        "## Counts".to_string(),
        format!("- Total lines inspected: {}", report.total_lines_inspected),
        format!("- Valid parsed rows: {}", report.valid_parsed_rows),
        format!("- Accepted rows: {}", report.accepted_rows),
        format!("- Rejected rows total: {}", report.rejected_rows_total),
        String::new(),
        "### Rejections by reason".to_string(),
    ];

    for (reason, count) in report.rejected_rows_by_reason.iter() {
        lines.push(format!("- {reason}: {count}"));
    }

    lines.extend([
        String::new(),
        "## Duplicates and uniqueness".to_string(),
        format!("- Exact duplicate pairs: {}", report.exact_duplicate_pair_count),
        format!("- Duplicate English: {}", report.duplicate_english_count),
        format!("- Duplicate Somali: {}", report.duplicate_somali_count),
        format!(
            "- Duplicate Somali w/ different English: {}",
            report.duplicate_somali_different_english_count
        ),
        format!("- Unique English: {}", report.unique_english_count),
        format!("- Unique Somali: {}", report.unique_somali_count),
        String::new(),
        "## Score & length statistics".to_string(),
    ]);

    for (label, stats) in [
        ("LASER score", &report.laser_score_stats),
        ("English LID score", &report.english_lid_score_stats),
        ("Somali LID score", &report.somali_lid_score_stats),
        ("English length", &report.english_length_stats),
        ("Somali length", &report.somali_length_stats),
        ("Length ratio", &report.length_ratio_stats),
    ] {
        lines.push(format!("- {label}: {}", json_line(stats)));
    }

    lines.extend([
        String::new(),
        "## Quality flags".to_string(),
        format!(
            "- Whitespace tokens: {}",
            json_line(&report.whitespace_token_counts)
        ),
        format!("- Missing URLs: {}", json_line(&report.missing_url_counts)),
        format!("- Empty text: {}", json_line(&report.empty_text_counts)),
        format!(
            "- Identical English/Somali: {}",
            report.identical_english_somali_count
        ),
        format!("- Numeric-only pairs: {}", report.numeric_only_pair_count),
        format!("- URL-heavy pairs: {}", report.url_heavy_pair_count),
        format!("- HTML/XML remnants: {}", report.html_xml_remnant_count),
        format!(
            "- Replacement characters: {}",
            report.replacement_character_count
        ),
        format!(
            "- Suspicious control characters: {}",
            report.suspicious_control_character_count
        ),
        format!("- Repeated characters: {}", report.repeated_character_count),
        String::new(),
        "## Source distributions".to_string(),
        format!(
            "- English sources: {}",
            json_line(&report.english_source_distribution)
        ),
        format!(
            "- Somali sources: {}",
            json_line(&report.somali_source_distribution)
        ),
        format!(
            "- English URL domains: {}",
            json_line(&report.english_url_domain_distribution)
        ),
        format!(
            "- Somali URL domains: {}",
            json_line(&report.somali_url_domain_distribution)
        ),
        String::new(),
        "## Most frequent sentences".to_string(),
        "### English".to_string(),
    ]);

    for item in &report.most_frequent_english_sentences {
        lines.push(format!("- ({}) {:?}", item.count, item.text));
    }
    lines.push("### Somali".to_string());
    for item in &report.most_frequent_somali_sentences {
        lines.push(format!("- ({}) {:?}", item.count, item.text));
    }

    lines.push(String::new());
    lines.push("## Warnings".to_string());
    for warning in &report.warnings {
        lines.push(format!("- {warning}"));
    }
    lines.push(String::new());

    lines.join("\n")
}

fn optional(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn json_line<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nllb::record::parse_raw_line;

    fn row(english: &str, somali: &str, laser: f64) -> RawRow {
        parse_raw_line(
            &[
                english,
                somali,
                &laser.to_string(),
                "0.99",
                "0.98",
                "src-en",
                "_",
                "src-so",
                "_",
            ]
            .join("\t"),
        )
        .unwrap()
    }

    fn audit_over_scrambled_scores() -> Audit {
        let mut audit = Audit::new(5).unwrap();
        for index in 0..50u64 {
            // Non-monotonic scores, so the lowest/highest groups differ from
            // the first/last ones.
            let laser = 1.0 + ((index * 17) % 50) as f64 / 100.0;
            let row = row(
                &format!("english number {index}"),
                &format!("soomaali tiro {index}"),
                laser,
            );
            audit.add_valid(&row, &row.english, &row.somali);
            audit.record_accepted();
        }
        audit
    }

    #[test]
    fn describes_an_empty_column_as_null() {
        let stats = describe(&mut []);
        assert_eq!(stats.count, 0);
        assert!(stats.mean.is_none());
        assert!(stats.median.is_none());
    }

    #[test]
    fn describes_values_with_a_population_stdev() {
        let stats = describe(&mut [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        assert_eq!(stats.count, 8);
        assert_eq!(stats.min, Some(2.0));
        assert_eq!(stats.max, Some(9.0));
        assert_eq!(stats.mean, Some(5.0));
        assert_eq!(stats.median, Some(4.5));
        assert_eq!(stats.stdev, Some(2.0));
    }

    #[test]
    fn single_value_has_zero_stdev() {
        assert_eq!(describe(&mut [3.0]).stdev, Some(0.0));
    }

    #[test]
    fn sampling_is_deterministic() {
        let first = audit_over_scrambled_scores().sampler().records();
        let second = audit_over_scrambled_scores().sampler().records();
        assert_eq!(first, second);

        let groups: HashSet<Option<String>> = first
            .iter()
            .map(|record| record.sample_group.clone())
            .collect();
        for expected in [
            "first_10_valid",
            "last_10_valid",
            "lowest_10_laser",
            "highest_10_laser",
            "random_sample",
        ] {
            assert!(groups.contains(&Some(expected.to_string())), "{expected}");
        }
    }

    #[test]
    fn lowest_group_never_outranks_the_highest_group() {
        let audit = audit_over_scrambled_scores();
        let groups = audit.sampler().groups();
        let scores = |name: &str| -> Vec<f64> {
            groups
                .iter()
                .find(|(group, _)| *group == name)
                .map(|(_, records)| records.iter().map(|record| record.laser_score).collect())
                .unwrap()
        };

        let lowest = scores("lowest_10_laser");
        let highest = scores("highest_10_laser");
        assert_eq!(lowest.len(), 10);
        assert_eq!(highest.len(), 10);
        assert!(lowest.iter().cloned().fold(f64::MIN, f64::max)
            <= highest.iter().cloned().fold(f64::MAX, f64::min));
        // Displayed low-to-high and high-to-low respectively.
        assert!(lowest.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(highest.windows(2).all(|pair| pair[0] >= pair[1]));
    }

    #[test]
    fn report_carries_the_source_identity_and_warnings() {
        let mut audit = audit_over_scrambled_scores();
        let report = audit.to_report(Some(12345));

        assert_eq!(report.dataset_repository, "allenai/nllb");
        assert_eq!(report.language_pair, "eng_Latn-som_Latn");
        assert_eq!(report.official_source_url, official_url());
        assert_eq!(report.raw_compressed_size_bytes, Some(12345));
        assert_eq!(report.accepted_rows, 50);
        assert_eq!(report.unique_english_count, 50);
        assert_eq!(report.warnings.len(), 6);
    }

    #[test]
    fn report_round_trips_through_json_keeping_reason_order() {
        let mut audit = audit_over_scrambled_scores();
        audit.record_rejection(Rejection::DuplicatePair);
        let report = audit.to_report(None);

        let json = serde_json::to_string(&report).unwrap();
        let parsed: AuditReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, report);

        let reasons: Vec<&str> = parsed
            .rejected_rows_by_reason
            .iter()
            .map(|(reason, _)| reason)
            .collect();
        assert_eq!(reasons[0], "malformed row");
        assert_eq!(reasons.len(), Rejection::ALL.len());
        assert_eq!(parsed.rejected_rows_by_reason.get("duplicate pair"), Some(1));
        assert_eq!(parsed.rejected_rows_total, 1);
    }

    #[test]
    fn duplicates_are_counted_per_side_and_per_pair() {
        let mut audit = Audit::new(0).unwrap();
        for (english, somali) in [("one", "isku mid"), ("two", "isku mid"), ("two", "isku mid")] {
            let row = row(english, somali, 1.0);
            audit.add_valid(&row, english, somali);
        }

        let report = audit.to_report(None);
        assert_eq!(report.exact_duplicate_pair_count, 1);
        assert_eq!(report.unique_somali_count, 1);
        assert_eq!(report.duplicate_somali_count, 2);
        // The first English seen for a Somali sentence stays the reference, so
        // both later rows count as "same Somali, different English".
        assert_eq!(report.duplicate_somali_different_english_count, 2);
        assert_eq!(report.unique_english_count, 2);
    }

    #[test]
    fn quality_flags_fire_on_the_expected_text() {
        let mut audit = Audit::new(0).unwrap();
        let rows = [
            row("visit https://example.test", "booqo www.example.test", 1.0),
            row("<p>hello</p>", "&amp; salaan", 1.0),
            row("bad \u{FFFD} text", "qoraal", 1.0),
            row("aaaaaaaaaaaa", "bbbbbbbbbbbb", 1.0),
            row("123 456", "789 000", 1.0),
            row("isku mid", "isku mid", 1.0),
        ];
        for row in &rows {
            audit.add_valid(row, &row.english, &row.somali);
        }

        let report = audit.to_report(None);
        assert_eq!(report.url_heavy_pair_count, 1);
        assert_eq!(report.html_xml_remnant_count, 1);
        assert_eq!(report.replacement_character_count, 1);
        assert_eq!(report.repeated_character_count, 1);
        assert_eq!(report.numeric_only_pair_count, 1);
        assert_eq!(report.identical_english_somali_count, 1);
    }

    #[test]
    fn empty_sides_are_counted_but_not_measured() {
        let mut audit = Audit::new(0).unwrap();
        let row = row("", "salaan", 1.0);
        audit.add_valid(&row, "", "salaan");

        let report = audit.to_report(None);
        assert_eq!(report.empty_text_counts.english, 1);
        assert_eq!(report.laser_score_stats.count, 0);
        assert_eq!(report.unique_somali_count, 1);
    }

    #[test]
    fn normalization_examples_record_only_changed_rows() {
        let mut audit = Audit::new(0).unwrap();
        let unchanged = row("hello", "salaan", 1.0);
        audit.add_valid(&unchanged, "hello", "salaan");
        let changed = row("hello", "salaan", 1.0);
        audit.add_valid(&changed, "hello  ", "salaan");

        let report = audit.to_report(None);
        assert_eq!(report.normalization_change_examples.len(), 1);
        assert_eq!(report.normalization_change_examples[0].original_english, "hello  ");
    }

    #[test]
    fn missing_urls_and_domains_are_split_out() {
        let mut audit = Audit::new(0).unwrap();
        let with_url = parse_raw_line(
            "a\tb\t1.0\t0.9\t0.9\tnews\thttps://Example.test:8080/x\tblog\t_",
        )
        .unwrap();
        audit.add_valid(&with_url, &with_url.english, &with_url.somali);

        let report = audit.to_report(None);
        assert_eq!(report.missing_url_counts.somali, 1);
        assert_eq!(report.missing_url_counts.english, 0);
        assert_eq!(
            report.english_url_domain_distribution.get("example.test:8080"),
            Some(1)
        );
        assert_eq!(report.english_source_distribution.get("news"), Some(1));
    }

    #[test]
    fn unparsable_urls_are_labelled_rather_than_dropped() {
        assert_eq!(domain("example.test/x"), "<unparsed>");
        assert_eq!(domain("https://example.test/x"), "example.test");
    }

    #[test]
    fn repeated_run_needs_ten_in_a_row() {
        assert!(!has_repeated_run(&"a".repeat(9)));
        assert!(has_repeated_run(&"a".repeat(10)));
        assert!(!has_repeated_run("abababab"));
    }

    #[test]
    fn markdown_lists_every_section() {
        let mut audit = audit_over_scrambled_scores();
        let markdown = render_markdown(&audit.to_report(Some(10)));
        for heading in [
            "# NLLB English-Somali audit report",
            "## Counts",
            "### Rejections by reason",
            "## Duplicates and uniqueness",
            "## Score & length statistics",
            "## Quality flags",
            "## Source distributions",
            "## Most frequent sentences",
            "## Warnings",
        ] {
            assert!(markdown.contains(heading), "missing {heading}");
        }
        assert!(markdown.contains("- Raw compressed size (bytes): 10"));
    }
}
