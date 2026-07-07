//! Aggregate per-stage stats into a single end-of-pipeline drops report.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;

use crate::report::print_kv;

const MERGE_REPORT: &str = "reports/01_merge_stats.json";
const CLEAN_REPORT: &str = "reports/02_clean_stats.json";
const LID_REPORT: &str = "reports/03_lid_stats.json";
const DEEP_CLEAN_REPORT: &str = "reports/04_deep_clean_stats.json";
const NEAR_DEDUP_REPORT: &str = "reports/05_near_dedup_stats.json";
const DEFAULT_OUTPUT_MD: &str = "reports/pipeline_drops.md";
const DEFAULT_OUTPUT_JSON: &str = "reports/pipeline_drops.json";

#[derive(Debug, Clone, Serialize)]
struct FunnelRow {
    stage: String,
    input: u64,
    kept: u64,
    dropped: u64,
    drop_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
struct DropRow {
    stage: String,
    reason: String,
    count: u64,
}

#[derive(Debug, Clone, Serialize)]
struct SourceDropRow {
    source: String,
    stage: String,
    reason: String,
    count: u64,
}

#[derive(Debug, Serialize)]
struct PipelineDropsReport {
    generated_at: String,
    stages_run: Vec<String>,
    funnel: Vec<FunnelRow>,
    raw_input: u64,
    final_kept: u64,
    total_removed: u64,
    overall_drop_rate: f64,
    all_drops: Vec<DropRow>,
    per_source_drops: Vec<SourceDropRow>,
    reject_sidecars: BTreeMap<String, String>,
    output_files: BTreeMap<String, String>,
}

fn read_json(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

fn map_u64(value: &Value) -> u64 {
    value.as_u64().unwrap_or(0)
}

fn map_f64(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

fn merge_drops(value: &Value) -> Vec<DropRow> {
    let mut rows = Vec::new();
    if let Some(map) = value.get("within_source_dup_drops").and_then(|v| v.as_object()) {
        let total: u64 = map.values().map(map_u64).sum();
        if total > 0 {
            rows.push(DropRow {
                stage: "merge".into(),
                reason: "within_source_dup".into(),
                count: total,
            });
        }
    }
    if let Some(map) = value.get("cross_source_dup_drops").and_then(|v| v.as_object()) {
        let total: u64 = map.values().map(map_u64).sum();
        if total > 0 {
            rows.push(DropRow {
                stage: "merge".into(),
                reason: "cross_source_dup".into(),
                count: total,
            });
        }
    }
    rows
}

fn drops_by_reason(value: &Value, stage: &str) -> Vec<DropRow> {
    let mut rows = Vec::new();
    if let Some(map) = value.get("drops_by_reason").and_then(|v| v.as_object()) {
        for (reason, count) in map {
            let count = map_u64(count);
            if count > 0 {
                rows.push(DropRow {
                    stage: stage.into(),
                    reason: reason.clone(),
                    count,
                });
            }
        }
    }
    rows
}

fn per_source_stage_drops(
    value: &Value,
    stage: &str,
    key: &str,
) -> Vec<SourceDropRow> {
    let mut rows = Vec::new();
    let Some(map) = value.get(key).and_then(|v| v.as_object()) else {
        return rows;
    };
    for (source, reasons) in map {
        if let Some(reason_map) = reasons.as_object() {
            for (reason, count) in reason_map {
                let count = map_u64(count);
                if count > 0 {
                    rows.push(SourceDropRow {
                        source: source.clone(),
                        stage: stage.into(),
                        reason: reason.clone(),
                        count,
                    });
                }
            }
        } else {
            let count = map_u64(reasons);
            if count > 0 {
                rows.push(SourceDropRow {
                    source: source.clone(),
                    stage: stage.into(),
                    reason: "dropped".into(),
                    count,
                });
            }
        }
    }
    rows
}

fn merge_per_source_drops(value: &Value) -> Vec<SourceDropRow> {
    let mut rows = Vec::new();
    for (key, reason) in [
        ("within_source_dup_drops", "within_source_dup"),
        ("cross_source_dup_drops", "cross_source_dup"),
    ] {
        if let Some(map) = value.get(key).and_then(|v| v.as_object()) {
            for (source, count) in map {
                let count = map_u64(count);
                if count > 0 {
                    rows.push(SourceDropRow {
                        source: source.clone(),
                        stage: "merge".into(),
                        reason: reason.into(),
                        count,
                    });
                }
            }
        }
    }
    rows
}

fn funnel_from_reports(stages: &[&str], reports: &BTreeMap<String, Value>) -> Vec<FunnelRow> {
    let mut funnel = Vec::new();

    if stages.contains(&"merge") {
        if let Some(v) = reports.get("merge") {
            let input = map_u64(v.get("total_input_docs").unwrap_or(&Value::Null));
            let kept = map_u64(v.get("total_output_docs").unwrap_or(&Value::Null));
            let dropped = map_u64(v.get("total_dropped").unwrap_or(&Value::Null));
            funnel.push(FunnelRow {
                stage: "merge".into(),
                input,
                kept,
                dropped,
                drop_rate: map_f64(v.get("drop_rate").unwrap_or(&Value::Null)),
            });
        }
    }

    for (stage, key_in, key_out, key_drop) in [
        ("clean", "input_docs", "output_docs", "rejected_docs"),
        ("lid", "input_docs", "output_docs", "rejected_docs"),
        ("deep_clean", "input_docs", "output_docs", "rejected_docs"),
        ("near_dedup", "input_docs", "output_docs", "removed"),
    ] {
        if !stages.contains(&stage) {
            continue;
        }
        if let Some(v) = reports.get(stage) {
            let input = map_u64(v.get(key_in).unwrap_or(&Value::Null));
            let kept = map_u64(v.get(key_out).unwrap_or(&Value::Null));
            let dropped = map_u64(v.get(key_drop).unwrap_or(&Value::Null));
            funnel.push(FunnelRow {
                stage: stage.into(),
                input,
                kept,
                dropped,
                drop_rate: map_f64(v.get("drop_rate").unwrap_or(&Value::Null)),
            });
        }
    }

    funnel
}

/// Build and write the consolidated pipeline drops report.
pub fn write_pipeline_drops_report(
    stages: &[String],
    md_path: &Path,
    json_path: &Path,
) -> Result<()> {
    let stage_set: Vec<&str> = stages.iter().map(String::as_str).collect();
    let mut reports = BTreeMap::new();

    let paths = [
        ("merge", MERGE_REPORT),
        ("clean", CLEAN_REPORT),
        ("lid", LID_REPORT),
        ("deep_clean", DEEP_CLEAN_REPORT),
        ("near_dedup", NEAR_DEDUP_REPORT),
    ];

    for (stage, path) in paths {
        if !stage_set.contains(&stage) {
            continue;
        }
        let path = Path::new(path);
        if path.exists() {
            reports.insert(stage.to_string(), read_json(path)?);
        }
    }

    let funnel = funnel_from_reports(&stage_set, &reports);
    let raw_input = funnel.first().map(|r| r.input).unwrap_or(0);
    let final_kept = funnel.last().map(|r| r.kept).unwrap_or(0);
    let total_removed = raw_input.saturating_sub(final_kept);
    let overall_drop_rate = if raw_input == 0 {
        0.0
    } else {
        total_removed as f64 / raw_input as f64
    };

    let mut all_drops = Vec::new();
    let mut per_source_drops = Vec::new();

    if let Some(v) = reports.get("merge") {
        all_drops.extend(merge_drops(v));
        per_source_drops.extend(merge_per_source_drops(v));
    }
    if let Some(v) = reports.get("clean") {
        all_drops.extend(drops_by_reason(v, "clean"));
        per_source_drops.extend(per_source_stage_drops(
            v,
            "clean",
            "per_source_drops_by_reason",
        ));
        // fallback if only per_source_rejected exists
        if per_source_drops.iter().all(|r| r.stage != "clean") {
            per_source_drops.extend(per_source_stage_drops(
                v,
                "clean",
                "per_source_rejected",
            ));
        }
    }
    if let Some(v) = reports.get("lid") {
        all_drops.extend(drops_by_reason(v, "lid"));
        per_source_drops.extend(per_source_stage_drops(v, "lid", "per_source_rejected"));
    }
    if let Some(v) = reports.get("deep_clean") {
        all_drops.extend(drops_by_reason(v, "deep_clean"));
        per_source_drops.extend(per_source_stage_drops(
            v,
            "deep_clean",
            "per_source_drops_by_reason",
        ));
        if per_source_drops.iter().all(|r| r.stage != "deep_clean") {
            per_source_drops.extend(per_source_stage_drops(
                v,
                "deep_clean",
                "per_source_rejected",
            ));
        }
    }
    if let Some(v) = reports.get("near_dedup") {
        all_drops.extend(drops_by_reason(v, "near_dedup"));
        per_source_drops.extend(per_source_stage_drops(
            v,
            "near_dedup",
            "per_source_removed",
        ));
    }

    let mut reject_sidecars = BTreeMap::new();
    let mut output_files = BTreeMap::new();
    if let Some(v) = reports.get("merge") {
        if let Some(p) = v.get("output_file").and_then(|x| x.as_str()) {
            output_files.insert("merge".into(), p.into());
        }
    }
    for (stage, sidecar_key) in [
        ("clean", "reject_sidecar"),
        ("lid", "reject_sidecar"),
        ("deep_clean", "reject_sidecar"),
        ("near_dedup", "reject_sidecar"),
    ] {
        if let Some(v) = reports.get(stage) {
            if let Some(p) = v.get("output_file").and_then(|x| x.as_str()) {
                output_files.insert(stage.into(), p.into());
            }
            if let Some(p) = v.get(sidecar_key).and_then(|x| x.as_str()) {
                reject_sidecars.insert(stage.into(), p.into());
            }
        }
    }

    let report = PipelineDropsReport {
        generated_at: Utc::now().to_rfc3339(),
        stages_run: stages.to_owned(),
        funnel,
        raw_input,
        final_kept,
        total_removed,
        overall_drop_rate,
        all_drops,
        per_source_drops,
        reject_sidecars,
        output_files,
    };

    if let Some(parent) = json_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = md_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(&report)?;
    fs::write(json_path, &json)?;
    fs::write(md_path, markdown_body(&report))?;

    Ok(())
}

fn markdown_body(report: &PipelineDropsReport) -> String {
    let mut md = String::new();
    md.push_str("# Pipeline — Dropped / Removed Summary\n\n");
    md.push_str(&format!("- Generated: {}\n", report.generated_at));
    md.push_str(&format!(
        "- Stages run: {}\n",
        report.stages_run.join(" → ")
    ));
    md.push_str(&format!(
        "- **{} raw → {} final** ({} removed, {:.2}% overall)\n\n",
        report.raw_input,
        report.final_kept,
        report.total_removed,
        report.overall_drop_rate * 100.0
    ));

    md.push_str("## Funnel\n\n");
    md.push_str("| Stage | Input | Kept | Dropped | Drop % |\n|---|---:|---:|---:|---:|\n");
    for row in &report.funnel {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {:.2}% |\n",
            row.stage,
            row.input,
            row.kept,
            row.dropped,
            row.drop_rate * 100.0
        ));
    }
    if !report.funnel.is_empty() {
        md.push_str(&format!(
            "| **final** | | **{}** | | |\n",
            report.final_kept
        ));
    }

    md.push_str("\n## All drops by stage and reason\n\n");
    if report.all_drops.is_empty() {
        md.push_str("_No records were dropped in the stages that ran._\n");
    } else {
        md.push_str("| Stage | Reason | Count |\n|---|---|---:|\n");
        for row in &report.all_drops {
            md.push_str(&format!(
                "| {} | {} | {} |\n",
                row.stage, row.reason, row.count
            ));
        }
        let total: u64 = report.all_drops.iter().map(|r| r.count).sum();
        md.push_str(&format!("| **total** | | **{total}** |\n"));
    }

    md.push_str("\n## Per source\n\n");
    if report.per_source_drops.is_empty() {
        md.push_str("_No per-source drop breakdown available._\n");
    } else {
        md.push_str("| Source | Stage | Reason | Count |\n|---|---|---|---:|\n");
        for row in &report.per_source_drops {
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                row.source, row.stage, row.reason, row.count
            ));
        }
    }

    if !report.reject_sidecars.is_empty() {
        md.push_str("\n## Reject sidecars\n\n");
        md.push_str("Full dropped records (inspect with `jq`):\n\n");
        for (stage, path) in &report.reject_sidecars {
            md.push_str(&format!("- **{stage}**: `{path}`\n"));
        }
        md.push_str("\n```bash\n");
        md.push_str("# Count drops by reason across all sidecars\n");
        for (stage, path) in &report.reject_sidecars {
            md.push_str(&format!(
                "echo \"=== {stage} ===\" && jq -r '.quality.flags[0]' {path} 2>/dev/null | sort | uniq -c\n"
            ));
        }
        md.push_str("```\n");
    }

    if !report.output_files.is_empty() {
        md.push_str("\n## Output files\n\n");
        for (stage, path) in &report.output_files {
            md.push_str(&format!("- **{stage}**: `{path}`\n"));
        }
    }

    md.push_str("\n## Stage reports\n\n");
    md.push_str("| Stage | JSON | Markdown |\n|---|---|---|\n");
    for (stage, json) in [
        ("merge", MERGE_REPORT),
        ("clean", CLEAN_REPORT),
        ("lid", LID_REPORT),
        ("deep_clean", DEEP_CLEAN_REPORT),
        ("near_dedup", NEAR_DEDUP_REPORT),
    ] {
        if report.stages_run.iter().any(|s| s == stage) {
            let md_path = PathBuf::from(json).with_extension("md");
            md.push_str(&format!(
                "| {stage} | `{json}` | `{md}` |\n",
                md = md_path.display()
            ));
        }
    }

    md
}

pub fn default_md_path() -> PathBuf {
    PathBuf::from(DEFAULT_OUTPUT_MD)
}

pub fn default_json_path() -> PathBuf {
    PathBuf::from(DEFAULT_OUTPUT_JSON)
}

pub fn print_summary_paths(md_path: &Path, json_path: &Path) {
    print_kv("drops report (md)", md_path.display());
    print_kv("drops report (json)", json_path.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_markdown_from_smoke_stats() {
        let report = PipelineDropsReport {
            generated_at: "2026-01-01T00:00:00Z".into(),
            stages_run: vec!["merge".into(), "clean".into()],
            funnel: vec![
                FunnelRow {
                    stage: "merge".into(),
                    input: 100,
                    kept: 90,
                    dropped: 10,
                    drop_rate: 0.1,
                },
                FunnelRow {
                    stage: "clean".into(),
                    input: 90,
                    kept: 85,
                    dropped: 5,
                    drop_rate: 5.0 / 90.0,
                },
            ],
            raw_input: 100,
            final_kept: 85,
            total_removed: 15,
            overall_drop_rate: 0.15,
            all_drops: vec![
                DropRow {
                    stage: "merge".into(),
                    reason: "within_source_dup".into(),
                    count: 10,
                },
                DropRow {
                    stage: "clean".into(),
                    reason: "too_short".into(),
                    count: 5,
                },
            ],
            per_source_drops: vec![],
            reject_sidecars: BTreeMap::new(),
            output_files: BTreeMap::new(),
        };
        let md = markdown_body(&report);
        assert!(md.contains("100 raw → 85 final"));
        assert!(md.contains("too_short"));
    }
}
