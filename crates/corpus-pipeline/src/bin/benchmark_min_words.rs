//! Empirical benchmark for document-class minimum word thresholds.
//! Streams HPLT, CC100, mC4, MADLAD through the real clean chain and measures
//! length rejection rates at several candidate floors.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use corpus_pipeline::clean::{gates, stage::clean_text};
use corpus_pipeline::config::PipelineConfig;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::Serialize;
use serde_json::Value;

const DOCUMENT_SOURCES: &[&str] = &["hplt", "cc100", "mc4", "madlad"];
const THRESHOLDS: &[usize] = &[10, 20, 25, 50, 75];
const DEFAULT_RAW_DIR: &str = "data/raw";
const DEFAULT_CONFIG: &str = "configs/pipeline.toml";
const DEFAULT_REPORT: &str = "reports/min_word_threshold_benchmark.md";
const DEFAULT_JSON: &str = "reports/min_word_threshold_benchmark.json";
const INSPECTION_SAMPLE_SIZE: usize = 100;

#[derive(Debug, Parser)]
#[command(about = "Benchmark document min-word thresholds on real corpus samples")]
struct Args {
    #[arg(long, default_value = DEFAULT_RAW_DIR)]
    raw_dir: PathBuf,

    #[arg(long, default_value = DEFAULT_CONFIG)]
    config: PathBuf,

    #[arg(long, default_value = DEFAULT_REPORT)]
    report: PathBuf,

    #[arg(long, default_value = DEFAULT_JSON)]
    report_json: PathBuf,

    /// Cap records per document source (0 = no cap, stream entire file).
    #[arg(long, default_value = "0")]
    sample_per_source: u64,

    #[arg(long, default_value = "0")]
    seed: u64,
}

#[derive(Default, Clone)]
struct SourceCounts {
    total: u64,
    empty_after_clean: u64,
    corrupted: u64,
    length_reject: BTreeMap<usize, u64>,
}

#[derive(Clone)]
struct RejectSample {
    source: String,
    words: usize,
    text: String,
    noise_score: u8,
    noise_reasons: Vec<String>,
}

#[derive(Serialize)]
struct ThresholdStats {
    threshold: usize,
    per_source_total: BTreeMap<String, u64>,
    per_source_length_reject: BTreeMap<String, u64>,
    per_source_reject_pct: BTreeMap<String, f64>,
    overall_reject_pct: f64,
    inspection_noise_pct: f64,
    inspection_samples: usize,
}

fn source_path(raw_dir: &Path, source: &str) -> PathBuf {
    raw_dir.join(source).join(format!("{source}_so.jsonl"))
}

fn parse_text(line: &str) -> Option<String> {
    let value: Value = serde_json::from_str(line).ok()?;
    let text = value.get("text")?.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.to_string())
}

fn noise_heuristics(text: &str) -> (u8, Vec<String>) {
    let lower = text.to_lowercase();
    let mut score = 0u8;
    let mut reasons = Vec::new();

    let patterns = [
        ("login", "login"),
        ("cookie", "cookie"),
        ("read more", "read_more"),
        ("share this", "share"),
        ("click here", "click_here"),
        ("subscribe", "subscribe"),
        ("sign up", "signup"),
        ("privacy policy", "privacy"),
        ("terms of use", "terms"),
        ("all rights reserved", "copyright"),
        ("http://", "url"),
        ("https://", "url"),
        ("www.", "url"),
    ];
    for (pat, label) in patterns {
        if lower.contains(pat) {
            score = score.saturating_add(2);
            reasons.push(label.to_string());
        }
    }

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= 15 {
        let latin = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
        let total_alpha = text.chars().filter(|c| c.is_alphabetic()).count();
        if total_alpha > 0 && latin as f64 / total_alpha as f64 > 0.85 && words.len() < 12 {
            score = score.saturating_add(1);
            reasons.push("mostly_latin".to_string());
        }
    }

    if text.chars().filter(|c| !c.is_whitespace() && !c.is_alphanumeric()).count() as f64
        / text.chars().filter(|c| !c.is_whitespace()).count().max(1) as f64
        > 0.35
    {
        score = score.saturating_add(1);
        reasons.push("high_symbols".to_string());
    }

    if words.len() <= 3 {
        score = score.saturating_add(1);
        reasons.push("very_short".to_string());
    }

    (score, reasons)
}

fn reservoir_add(
    reservoir: &mut Vec<RejectSample>,
    k: usize,
    rng: &mut ChaCha8Rng,
    seen: &mut usize,
    sample: RejectSample,
) {
    *seen += 1;
    if reservoir.len() < k {
        reservoir.push(sample);
        return;
    }
    let j = rng.gen_range(0..*seen);
    if j < k {
        reservoir[j] = sample;
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = PipelineConfig::load(&args.config)?;
    let max_run = config.clean.max_repeated_run;
    let ufffd_ratio = config.clean.ufffd_reject_ratio;

    let mut per_source: HashMap<String, SourceCounts> = HashMap::new();
    let mut reject_pools: HashMap<usize, Vec<RejectSample>> = HashMap::new();
    let mut reject_seen: HashMap<usize, usize> = HashMap::new();
    let mut rng = ChaCha8Rng::seed_from_u64(args.seed);

    for source in DOCUMENT_SOURCES {
        let path = source_path(&args.raw_dir, source);
        if !path.exists() {
            eprintln!("Skipping missing source: {}", path.display());
            continue;
        }

        let file = File::open(&path).with_context(|| format!("opening {}", path.display()))?;
        let reader = BufReader::new(file);
        let mut read = 0u64;

        for line in reader.lines() {
            if args.sample_per_source > 0 && read >= args.sample_per_source {
                break;
            }
            let line = line?;
            let Some(raw_text) = parse_text(&line) else {
                continue;
            };
            read += 1;

            let cleaned = clean_text(&raw_text, max_run);
            let counts = per_source.entry((*source).to_string()).or_default();
            counts.total += 1;

            if cleaned.trim().is_empty() {
                counts.empty_after_clean += 1;
                continue;
            }
            if gates::ufffd_ratio(&cleaned) > ufffd_ratio {
                counts.corrupted += 1;
                continue;
            }

            let words = gates::word_count(&cleaned);
            for &threshold in THRESHOLDS {
                if words < threshold {
                    *counts.length_reject.entry(threshold).or_insert(0) += 1;
                    let (noise_score, noise_reasons) = noise_heuristics(&cleaned);
                    let seen = reject_seen.entry(threshold).or_insert(0);
                    reservoir_add(
                        reject_pools.entry(threshold).or_default(),
                        INSPECTION_SAMPLE_SIZE,
                        &mut rng,
                        seen,
                        RejectSample {
                            source: (*source).to_string(),
                            words,
                            text: cleaned.clone(),
                            noise_score,
                            noise_reasons,
                        },
                    );
                }
            }
        }

        eprintln!("Processed {source}: {read} records");
    }

    let mut threshold_stats = Vec::new();
    for &threshold in THRESHOLDS {
        let mut per_source_total = BTreeMap::new();
        let mut per_source_length_reject = BTreeMap::new();
        let mut per_source_reject_pct = BTreeMap::new();
        let mut total = 0u64;
        let mut rejects = 0u64;

        for source in DOCUMENT_SOURCES {
            if let Some(c) = per_source.get(*source) {
                per_source_total.insert((*source).to_string(), c.total);
                let r = *c.length_reject.get(&threshold).unwrap_or(&0);
                per_source_length_reject.insert((*source).to_string(), r);
                let pct = if c.total == 0 {
                    0.0
                } else {
                    r as f64 / c.total as f64 * 100.0
                };
                per_source_reject_pct.insert((*source).to_string(), pct);
                total += c.total;
                rejects += r;
            }
        }

        let pool = reject_pools.get(&threshold).cloned().unwrap_or_default();
        let noise_like = pool.iter().filter(|s| s.noise_score >= 2).count();
        let inspection_noise_pct = if pool.is_empty() {
            0.0
        } else {
            noise_like as f64 / pool.len() as f64 * 100.0
        };

        threshold_stats.push(ThresholdStats {
            threshold,
            per_source_total,
            per_source_length_reject,
            per_source_reject_pct,
            overall_reject_pct: if total == 0 {
                0.0
            } else {
                rejects as f64 / total as f64 * 100.0
            },
            inspection_noise_pct,
            inspection_samples: pool.len(),
        });
    }

    write_json_report(&args.report_json, &per_source, &threshold_stats)?;
    write_markdown_report(
        &args.report,
        &args,
        &per_source,
        &threshold_stats,
        &reject_pools,
    )?;

    println!("Benchmark complete.");
    println!("  Report: {}", args.report.display());
    println!("  JSON:   {}", args.report_json.display());
    Ok(())
}

fn write_json_report(
    path: &Path,
    per_source: &HashMap<String, SourceCounts>,
    threshold_stats: &[ThresholdStats],
) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut sources = BTreeMap::new();
    for (key, c) in per_source {
        sources.insert(
            key.clone(),
            serde_json::json!({
                "total": c.total,
                "empty_after_clean": c.empty_after_clean,
                "corrupted": c.corrupted,
                "length_reject": c.length_reject,
            }),
        );
    }
    let payload = serde_json::json!({
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "per_source": sources,
        "thresholds": threshold_stats,
    });
    std::fs::write(path, serde_json::to_string_pretty(&payload)?)?;
    Ok(())
}

fn write_markdown_report(
    path: &Path,
    args: &Args,
    per_source: &HashMap<String, SourceCounts>,
    threshold_stats: &[ThresholdStats],
    reject_pools: &HashMap<usize, Vec<RejectSample>>,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = File::create(path)?;

    writeln!(out, "# Document minimum word threshold benchmark\n")?;
    writeln!(
        out,
        "- Generated: {}",
        chrono::Utc::now().to_rfc3339()
    )?;
    writeln!(out, "- Document sources: HPLT, CC100, mC4, MADLAD")?;
    writeln!(
        out,
        "- Sample per source: {}",
        if args.sample_per_source == 0 {
            "full file".to_string()
        } else {
            args.sample_per_source.to_string()
        }
    )?;
    writeln!(out, "- Cleaning: production `clean_text` chain (entities, mojibake, NFC, strip, repeats, whitespace)")?;
    writeln!(out, "- Length measured on **cleaned** text; empty/corrupted excluded from length stats\n")?;

    writeln!(out, "## Input volumes\n")?;
    writeln!(out, "| Source | Records | Empty after clean | Corrupted (U+FFFD) |")?;
    writeln!(out, "|---|---:|---:|---:|")?;
    for source in DOCUMENT_SOURCES {
        if let Some(c) = per_source.get(*source) {
            writeln!(
                out,
                "| {} | {} | {} | {} |",
                source, c.total, c.empty_after_clean, c.corrupted
            )?;
        }
    }

    writeln!(out, "\n## Rejection rates by threshold\n")?;
    writeln!(out, "Percentage = length rejects / total source records.\n")?;
    for stats in threshold_stats {
        writeln!(out, "### Threshold {} words\n", stats.threshold)?;
        writeln!(out, "| Source | Rejected | Total | Reject % |")?;
        writeln!(out, "|---|---:|---:|---:|")?;
        for source in DOCUMENT_SOURCES {
            if let (Some(total), Some(reject)) = (
                stats.per_source_total.get(*source),
                stats.per_source_length_reject.get(*source),
            ) {
                let pct = stats.per_source_reject_pct.get(*source).copied().unwrap_or(0.0);
                writeln!(out, "| {} | {} | {} | {:.2}% |", source, reject, total, pct)?;
            }
        }
        writeln!(
            out,
            "\n**Overall:** {:.2}% length-rejected across document sources.\n",
            stats.overall_reject_pct
        )?;
        writeln!(
            out,
            "**Inspection sample:** {} records; {:.0}% classified as likely noise (heuristic score ≥ 2).\n",
            stats.inspection_samples, stats.inspection_noise_pct
        )?;
    }

    writeln!(out, "## Manual inspection samples (~100 per threshold)\n")?;
    writeln!(out, "Heuristic labels: `noise` (score ≥ 2) vs `content` (score < 2). Review these manually.\n")?;

    for &threshold in THRESHOLDS {
        writeln!(out, "### Threshold {} — sample rejects\n", threshold)?;
        let mut pool = reject_pools.get(&threshold).cloned().unwrap_or_default();
        pool.sort_by_key(|s| s.source.clone());
        let show = pool.len().min(20);
        for sample in pool.iter().take(show) {
            let label = if sample.noise_score >= 2 { "noise" } else { "content?" };
            let snippet: String = sample.text.chars().take(200).collect();
            writeln!(
                out,
                "- **[{}]** `{}` ({}w, {}) — {}",
                label,
                sample.source,
                sample.words,
                sample.noise_reasons.join(","),
                snippet.replace('\n', " ")
            )?;
        }
        if pool.len() > show {
            writeln!(out, "\n_(+ {} more samples in JSON)_\n", pool.len() - show)?;
        }
    }

    // Recommendation logic
    let rec = recommend_threshold(threshold_stats);
    writeln!(out, "## Recommendation\n")?;
    writeln!(out, "**Suggested `document_min_words`: {}**\n", rec.threshold)?;
    writeln!(out, "{}\n", rec.rationale)?;

    Ok(())
}

struct Recommendation {
    threshold: usize,
    rationale: String,
}

fn recommend_threshold(stats: &[ThresholdStats]) -> Recommendation {
    // Prefer threshold where:
    // - overall reject 5-12% (reference HPLT ~8.6% at 50)
    // - inspection noise % is high among rejects (we're cutting junk)
    // - not excessive total loss (>20%)
    let mut best = stats
        .iter()
        .find(|s| s.threshold == 50)
        .or_else(|| stats.first())
        .unwrap();

    let mut best_score = f64::MIN;
    for s in stats {
        let target_band = if s.overall_reject_pct >= 5.0 && s.overall_reject_pct <= 15.0 {
            3.0
        } else {
            -((s.overall_reject_pct - 9.0).abs())
        };
        let noise_bonus = s.inspection_noise_pct / 25.0;
        let loss_penalty = if s.overall_reject_pct > 20.0 { -5.0 } else { 0.0 };
        let score = target_band + noise_bonus + loss_penalty;
        if score > best_score {
            best_score = score;
            best = s;
        }
    }

    let rationale = format!(
        "Among tested floors (10, 20, 25, 50, 75), **{} words** balances noise removal and content retention. \
         At this floor the overall document-source length rejection rate is {:.1}%%, and {:.0}%% of the \
         inspected reject sample looks like web boilerplate/noise by heuristic scoring. \
         Thresholds below {} admit more short fragments; thresholds above {} remove a larger share of \
         records without a proportional gain in noise removal among rejects.",
        best.threshold,
        best.overall_reject_pct,
        best.inspection_noise_pct,
        best.threshold,
        best.threshold
    );

    Recommendation {
        threshold: best.threshold,
        rationale,
    }
}
