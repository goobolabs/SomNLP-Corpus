//! Terminal summaries and human-readable report helpers for pipeline stages.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs;
use std::path::Path;

use anyhow::Result;
use common::types::QualityFlag;

pub fn print_banner(title: &str) {
    println!();
    println!("{}", "=".repeat(56));
    println!("{title}");
    println!("{}", "=".repeat(56));
}

pub fn pct(n: u64, total: u64) -> String {
    if total == 0 {
        "0.00%".to_string()
    } else {
        format!("{:.2}%", n as f64 / total as f64 * 100.0)
    }
}

pub fn print_kv(key: &str, value: impl Display) {
    println!("  {key:<22} {value}");
}

pub fn print_drops_by_reason(reasons: &BTreeMap<String, u64>, total_rejected: u64) {
    if reasons.is_empty() && total_rejected == 0 {
        print_kv("drops by reason", "(none)");
        return;
    }
    println!("  drops by reason:");
    for (reason, count) in reasons {
        let share = if total_rejected == 0 {
            "0.00%".to_string()
        } else {
            pct(*count, total_rejected)
        };
        println!("    - {reason:<20} {count:>8}  ({share} of rejects)");
    }
}

pub fn print_per_source_flow(
    per_source_in: &BTreeMap<String, u64>,
    per_source_kept: &BTreeMap<String, u64>,
    per_source_rejected: Option<&BTreeMap<String, u64>>,
) {
    if per_source_in.is_empty() && per_source_kept.is_empty() {
        return;
    }
    println!("  per source:");
    println!(
        "    {:<10} {:>10} {:>10} {:>10} {:>8}",
        "source", "input", "kept", "rejected", "drop%"
    );
    let sources: Vec<_> = per_source_in
        .keys()
        .chain(per_source_kept.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for source in sources {
        let input = per_source_in.get(source).copied().unwrap_or(0);
        let kept = per_source_kept.get(source).copied().unwrap_or(0);
        let rejected = per_source_rejected
            .and_then(|m| m.get(source).copied())
            .unwrap_or(input.saturating_sub(kept));
        println!(
            "    {:<10} {:>10} {:>10} {:>10} {:>8}",
            source,
            input,
            kept,
            rejected,
            pct(rejected, input)
        );
    }
}

pub fn quality_flag_name(flag: &QualityFlag) -> String {
    serde_json::to_value(flag)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "unspecified".to_string())
}

pub fn print_inspect_hint(sidecar: &Path, fields: &str) {
    if sidecar.exists() {
        println!("  inspect rejects:");
        println!(
            "    jq -r '{fields}' {} | head -40",
            sidecar.display()
        );
    }
}

pub fn print_paths(output: &Path, report_json: &Path, reject_sidecar: Option<&Path>) {
    print_kv("output", output.display());
    print_kv("report (json)", report_json.display());
    if let Some(path) = reject_sidecar {
        if path.exists() {
            print_kv("reject sidecar", path.display());
        }
    }
}

/// Write a companion `.md` report (`reports/foo.json` → `reports/foo.md`).
pub fn write_markdown_companion(json_path: &Path, body: &str) -> Result<()> {
    let md_path = json_path.with_extension("md");
    if let Some(parent) = md_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&md_path, body)?;
    print_kv("report (md)", md_path.display());
    Ok(())
}
