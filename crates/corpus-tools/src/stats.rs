use std::collections::BTreeMap;

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub total_docs: u64,
    pub total_chars: u64,
    pub per_source: BTreeMap<String, u64>,
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
