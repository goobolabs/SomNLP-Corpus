use std::collections::BTreeMap;

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub total_docs: u64,
    pub total_chars: u64,
    pub per_source: BTreeMap<String, u64>,
}

/// Exact-dedup counters recorded during the merge stage.
#[derive(Debug, Default, Clone)]
pub struct DedupCounters {
    pub total_input: u64,
    pub total_kept: u64,
    pub within_source_dups: BTreeMap<String, u64>,
    pub cross_source_dups: BTreeMap<String, u64>,
    pub per_source_input: BTreeMap<String, u64>,
    pub per_source_kept: BTreeMap<String, u64>,
}

impl DedupCounters {
    pub fn record_input(&mut self, source: &str) {
        self.total_input += 1;
        *self.per_source_input.entry(source.to_string()).or_insert(0) += 1;
    }

    pub fn record_kept(&mut self, source: &str) {
        self.total_kept += 1;
        *self.per_source_kept.entry(source.to_string()).or_insert(0) += 1;
    }

    pub fn record_within_dup(&mut self, source: &str) {
        *self.within_source_dups.entry(source.to_string()).or_insert(0) += 1;
    }

    pub fn record_cross_dup(&mut self, source: &str) {
        *self.cross_source_dups.entry(source.to_string()).or_insert(0) += 1;
    }

    pub fn total_dropped(&self) -> u64 {
        self.total_input - self.total_kept
    }
}

impl Stats {
    pub fn avg_len(&self) -> f64 {
        if self.total_docs == 0 {
            0.0
        } else {
            self.total_chars as f64 / self.total_docs as f64
        }
    }

    pub fn record(&mut self, text: &str) {
        self.total_docs += 1;
        self.total_chars += text.chars().count() as u64;
    }

    pub fn record_source(&mut self, source: &str, text: &str) {
        self.record(text);
        *self.per_source.entry(source.to_string()).or_insert(0) += 1;
    }
}

pub fn format_number(value: u64) -> String {
    let s = value.to_string();
    let mut out = String::new();
    for (index, ch) in s.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

pub fn format_float(value: f64) -> String {
    let whole = value.trunc() as u64;
    let frac = ((value - whole as f64) * 100.0).round() as u64;
    format!("{}.{:02}", format_number(whole), frac)
}
