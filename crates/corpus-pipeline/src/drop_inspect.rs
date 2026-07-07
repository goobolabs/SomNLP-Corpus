//! Copy-paste commands to read dropped/removed record text after a pipeline run.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::report::print_banner;

/// Sidecar file holding dropped text for a pipeline stage.
#[derive(Debug, Clone, Copy)]
pub struct DropSidecar {
    pub stage: &'static str,
    pub path: &'static str,
    /// `merge` = `{reason, source, text}`; `corpus` = CorpusRecord reject sidecar.
    pub format: DropFormat,
}

#[derive(Debug, Clone, Copy)]
pub enum DropFormat {
    Merge,
    Corpus,
}

pub const ALL_DROPS: &[DropSidecar] = &[
    DropSidecar {
        stage: "merge",
        path: "data/merged/merged_so.dropped.jsonl",
        format: DropFormat::Merge,
    },
    DropSidecar {
        stage: "clean",
        path: "data/cleaned/cleaned_so.rejected.jsonl",
        format: DropFormat::Corpus,
    },
    DropSidecar {
        stage: "lid",
        path: "data/lid/lid_so.rejected.jsonl",
        format: DropFormat::Corpus,
    },
    DropSidecar {
        stage: "deep_clean",
        path: "data/deep_clean/deep_clean_so.rejected.jsonl",
        format: DropFormat::Corpus,
    },
    DropSidecar {
        stage: "near_dedup",
        path: "data/final/final_so.rejected.jsonl",
        format: DropFormat::Corpus,
    },
];

pub const INSPECT_SCRIPT: &str = "reports/inspect_drops.sh";

fn sidecars_for_stages(stages: &[String]) -> Vec<&'static DropSidecar> {
    ALL_DROPS
        .iter()
        .filter(|s| stages.iter().any(|run| run == s.stage))
        .collect()
}

fn line_count(path: &Path) -> Option<u64> {
    if !path.exists() {
        return None;
    }
    let out = std::process::Command::new("wc")
        .args(["-l", &path.to_string_lossy()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
}

pub fn merge_dropped_path_for(output: &Path) -> PathBuf {
    let stem = output
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "merged_so".into());
    output.with_file_name(format!("{stem}.dropped.jsonl"))
}

/// Print a menu of commands to view dropped texts (only for stages that ran).
pub fn print_inspect_menu(stages: &[String]) {
    print_banner("Inspect dropped texts");
    println!("  Run one stage:  bash reports/inspect_drops.sh <merge|clean|lid|deep_clean|near_dedup>");
    println!("  Run all:        bash reports/inspect_drops.sh\n");

    for sidecar in sidecars_for_stages(stages) {
        let path = Path::new(sidecar.path);
        let count = line_count(path);
        let count_label = match count {
            Some(n) if n > 0 => format!("{n} records"),
            Some(_) => "0 drops".into(),
            None => "0 drops".into(),
        };

        println!("  [{}] {} — {}", sidecar.stage, sidecar.path, count_label);
        if count.unwrap_or(0) == 0 {
            println!("    (nothing to inspect — stage kept all records)\n");
            continue;
        }
        for (label, cmd) in commands_for(sidecar) {
            println!("    {label:<28} {cmd}");
        }
        println!();
    }

    println!("  [all]  Run every inspect command:");
    println!("    bash {INSPECT_SCRIPT}");
    println!();
}

fn commands_for(sidecar: &DropSidecar) -> Vec<(&'static str, String)> {
    let p = sidecar.path;
    match sidecar.format {
        DropFormat::Merge => vec![
            ("count", format!("wc -l {p}")),
            (
                "reasons",
                format!("jq -r '.reason' {p} | sort | uniq -c"),
            ),
            (
                "preview (30)",
                format!("jq -r '[.reason, .source, .text] | @tsv' {p} | head -30"),
            ),
            (
                "browse all",
                format!("jq -r '[.reason, .source, .text] | @tsv' {p} | less"),
            ),
            (
                "random 10",
                format!("shuf -n 10 {p} | jq -r '[.reason, .text] | @tsv'"),
            ),
            (
                "cross-source only",
                format!(
                    "jq -r 'select(.reason==\"cross_source_dup\") | [.kept_source, .source, .text] | @tsv' {p} | head -30"
                ),
            ),
        ],
        DropFormat::Corpus => vec![
            ("count", format!("wc -l {p}")),
            (
                "reasons",
                format!("jq -r '.quality.flags[0]' {p} | sort | uniq -c"),
            ),
            (
                "preview (30)",
                format!(
                    "jq -r '[.quality.flags[0], .provenance.source, .text] | @tsv' {p} | head -30"
                ),
            ),
            (
                "browse all",
                format!(
                    "jq -r '[.quality.flags[0], .provenance.source, .text] | @tsv' {p} | less"
                ),
            ),
            (
                "random 10",
                format!(
                    "shuf -n 10 {p} | jq -r '[.quality.flags[0], .text] | @tsv'"
                ),
            ),
            (
                "full record",
                format!("jq '.' {p} | head -60"),
            ),
        ],
    }
}

/// Write `reports/inspect_drops.sh` — runnable script with all inspect commands.
pub fn write_inspect_script(stages: &[String]) -> Result<PathBuf> {
    let path = PathBuf::from(INSPECT_SCRIPT);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut script = String::from("#!/usr/bin/env bash\n");
    script.push_str("# Inspect dropped/removed texts. Run from anywhere:\n");
    script.push_str("#   bash reports/inspect_drops.sh\n");
    script.push_str("#   bash reports/inspect_drops.sh clean\n\n");
    script.push_str("set -euo pipefail\n");
    script.push_str("ROOT=\"$(cd \"$(dirname \"$0\")/..\" && pwd)\"\n");
    script.push_str("cd \"$ROOT\"\n");
    script.push_str("STAGE=\"${1:-all}\"\n\n");

    for sidecar in ALL_DROPS {
        let p = sidecar.path;
        script.push_str(&format!(
            "run_{}() {{\n  echo \"=== {} ===\"\n  local f=\"{p}\"\n",
            sidecar.stage, sidecar.stage
        ));
        script.push_str(
            "  if [[ ! -f $f ]]; then echo \"  (no file — nothing dropped)\"; return; fi\n",
        );
        script.push_str("  echo \"  file: $f ($(wc -l < \"$f\") lines)\"\n  echo\n");
        for (label, cmd) in commands_for(sidecar) {
            script.push_str(&format!("  echo \"--- {label} ---\"\n  {cmd}\n  echo\n"));
        }
        script.push_str("}\n\n");
    }

    script.push_str("case \"$STAGE\" in\n");
    for sidecar in ALL_DROPS {
        script.push_str(&format!(
            "  {}|{}_drops) run_{} ;;\n",
            sidecar.stage, sidecar.stage, sidecar.stage
        ));
    }
    script.push_str("  all)\n");
    for sidecar in sidecars_for_stages(stages) {
        script.push_str(&format!("    run_{}\n", sidecar.stage));
    }
    script.push_str("    ;;\n");
    script.push_str("  *)\n");
    script.push_str("    echo \"Unknown stage: $STAGE\"\n");
    script.push_str(
        "    echo \"Usage: bash reports/inspect_drops.sh [merge|clean|lid|deep_clean|near_dedup|all]\"\n",
    );
    script.push_str("    exit 1\n");
    script.push_str("    ;;\n");
    script.push_str("esac\n");

    fs::write(&path, &script)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)?;
    }
    Ok(path)
}
