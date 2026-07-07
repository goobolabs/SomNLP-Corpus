//! Live terminal progress for long-running pipeline stages.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

pub fn format_number(value: u64) -> String {
    let s = value.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

/// Count JSONL lines (for progress bar totals). Uses `wc -l` when available.
pub fn count_jsonl_lines(path: &Path) -> Option<u64> {
    if !path.exists() {
        return None;
    }
    if let Ok(out) = Command::new("wc").args(["-l", &path.to_string_lossy()]).output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(n) = text.split_whitespace().next() {
                return n.parse().ok();
            }
        }
    }
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    Some(reader.lines().count() as u64)
}

/// Spinner or bar for record-at-a-time processing.
pub struct RecordProgress {
    bar: ProgressBar,
    started: Instant,
}

impl RecordProgress {
    pub fn start(label: &str, total: Option<u64>) -> Self {
        let bar = match total {
            Some(n) if n > 0 => {
                let bar = ProgressBar::new(n);
                Self::configure_bar(&bar);
                bar.set_style(
                    ProgressStyle::with_template(
                        "{spinner:.green} {msg} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {elapsed_precise}",
                    )
                    .expect("progress template")
                    .progress_chars("█▓░"),
                );
                bar
            }
            _ => {
                let bar = ProgressBar::new_spinner();
                Self::configure_bar(&bar);
                bar.set_style(
                    ProgressStyle::with_template("{spinner:.green} {msg} {pos} records {elapsed_precise}")
                        .expect("spinner template"),
                );
                bar.enable_steady_tick(std::time::Duration::from_millis(100));
                bar
            }
        };
        bar.set_draw_target(ProgressDrawTarget::stderr());
        bar.set_message(label.to_string());
        Self {
            bar,
            started: Instant::now(),
        }
    }

    fn configure_bar(bar: &ProgressBar) {
        bar.set_draw_target(ProgressDrawTarget::stderr());
    }

    pub fn inc(&self) {
        self.bar.inc(1);
    }

    pub fn set_message(&self, msg: impl AsRef<str>) {
        self.bar.set_message(msg.as_ref().to_string());
    }

    pub fn finish(&self, summary: impl AsRef<str>) {
        let elapsed = self.started.elapsed();
        self.bar.finish_with_message(format!(
            "{} — {} ({:.1}s)",
            summary.as_ref(),
            format_number(self.bar.position()),
            elapsed.as_secs_f64()
        ));
    }
}

/// Multi-step phase progress (e.g. near-dedup clustering).
pub struct PhaseProgress {
    bar: ProgressBar,
    step: usize,
    total_steps: usize,
}

impl PhaseProgress {
    pub fn new(total_steps: usize, title: &str) -> Self {
        let bar = ProgressBar::new(total_steps as u64);
        bar.set_draw_target(ProgressDrawTarget::stderr());
        bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} {msg} [step {pos}/{len}] {elapsed_precise}",
            )
            .expect("phase template"),
        );
        bar.set_message(title.to_string());
        Self {
            bar,
            step: 0,
            total_steps,
        }
    }

    pub fn next(&mut self, label: impl AsRef<str>) {
        self.step += 1;
        self.bar.set_position(self.step as u64);
        self.bar.set_message(format!(
            "[{}/{}] {}",
            self.step,
            self.total_steps,
            label.as_ref()
        ));
    }

    pub fn finish(&self, summary: impl AsRef<str>) {
        self.bar.finish_with_message(summary.as_ref().to_string());
    }
}

/// Print pipeline stage checklist before/after each stage.
pub fn print_stage_checklist(stages: &[String], completed: &[String], current: Option<&str>) {
    println!();
    println!("{}", "─".repeat(56));
    println!("Pipeline progress");
    println!("{}", "─".repeat(56));
    for (i, stage) in stages.iter().enumerate() {
        let n = i + 1;
        let label = stage_label(stage.as_str());
        let icon = if completed.iter().any(|s| s == stage) {
            "✓"
        } else if current == Some(stage.as_str()) {
            "▶"
        } else {
            " "
        };
        let status = if completed.iter().any(|s| s == stage) {
            "done"
        } else if current == Some(stage.as_str()) {
            "running"
        } else {
            "pending"
        };
        println!("  [{icon}] {n}/{total} {label:<28} {status}", total = stages.len());
    }
    println!("{}", "─".repeat(56));
}

fn stage_label(stage: &str) -> String {
    match stage {
        "merge" => "Merge + exact dedup".into(),
        "clean" => "Clean".into(),
        "lid" => "Language ID".into(),
        "deep_clean" => "Deep clean (v0.2)".into(),
        "near_dedup" => "Near dedup".into(),
        other => other.into(),
    }
}

pub fn stage_report_path(stage: &str) -> Option<&'static str> {
    match stage {
        "merge" => Some("reports/01_merge_stats.json"),
        "clean" => Some("reports/02_clean_stats.json"),
        "lid" => Some("reports/03_lid_stats.json"),
        "deep_clean" => Some("reports/04_deep_clean_stats.json"),
        "near_dedup" => Some("reports/05_near_dedup_stats.json"),
        _ => None,
    }
}

/// One-line summary from a stage stats JSON file.
pub fn stage_summary_line(stage: &str) -> Option<String> {
    let path = stage_report_path(stage)?;
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;

    match stage {
        "merge" => {
            let input = v["total_input_docs"].as_u64()?;
            let kept = v["total_output_docs"].as_u64()?;
            let dropped = v["total_dropped"].as_u64().unwrap_or(input.saturating_sub(kept));
            Some(format!(
                "{input_fmt} in → {kept_fmt} kept, {dropped_fmt} dropped ({pct})",
                input_fmt = format_number(input),
                kept_fmt = format_number(kept),
                dropped_fmt = format_number(dropped),
                pct = crate::report::pct(dropped, input),
            ))
        }
        "clean" => {
            let input = v["input_docs"].as_u64()?;
            let kept = v["output_docs"].as_u64()?;
            let rejected = v["rejected_docs"].as_u64()?;
            Some(format!(
                "{input_fmt} in → {kept_fmt} kept, {rejected_fmt} rejected ({pct})",
                input_fmt = format_number(input),
                kept_fmt = format_number(kept),
                rejected_fmt = format_number(rejected),
                pct = crate::report::pct(rejected, input),
            ))
        }
        "lid" => {
            let input = v["input_docs"].as_u64()?;
            let kept = v["output_docs"].as_u64()?;
            let rejected = v["rejected_docs"].as_u64()?;
            let backend = v["backend"].as_str().unwrap_or("?");
            Some(format!(
                "{input_fmt} in → {kept_fmt} kept, {rejected_fmt} rejected ({pct}) [{backend}]",
                input_fmt = format_number(input),
                kept_fmt = format_number(kept),
                rejected_fmt = format_number(rejected),
                pct = crate::report::pct(rejected, input),
            ))
        }
        "deep_clean" => {
            let input = v["input_docs"].as_u64()?;
            let kept = v["output_docs"].as_u64()?;
            let rejected = v["rejected_docs"].as_u64()?;
            Some(format!(
                "{input_fmt} in → {kept_fmt} kept, {rejected_fmt} rejected ({pct})",
                input_fmt = format_number(input),
                kept_fmt = format_number(kept),
                rejected_fmt = format_number(rejected),
                pct = crate::report::pct(rejected, input),
            ))
        }
        "near_dedup" => {
            let input = v["input_docs"].as_u64()?;
            let kept = v["output_docs"].as_u64()?;
            let removed = v["removed"].as_u64().unwrap_or(0);
            Some(format!(
                "{input_fmt} in → {kept_fmt} kept, {removed_fmt} near-dup removed ({pct})",
                input_fmt = format_number(input),
                kept_fmt = format_number(kept),
                removed_fmt = format_number(removed),
                pct = crate::report::pct(removed, input),
            ))
        }
        _ => None,
    }
}

pub fn print_completed_stages(_stages: &[String], completed: &[String]) {
    if completed.is_empty() {
        return;
    }
    println!();
    for stage in completed {
        let label = stage_label(stage.as_str());
        let summary = stage_summary_line(stage).unwrap_or_else(|| "complete".into());
        println!("  ✓ {label}: {summary}");
    }
}
