#!/usr/bin/env bash
# Run run_pipeline one stage at a time and, after each stage succeeds and writes its
# report, delete the previous stage's main JSONL (it is fully superseded). Reject and
# dropped sidecars are kept for drop inspection; data/raw is never touched here.
set -u
cd "$(dirname "$0")/../.."
LOG=reports/full_pipeline.log
CONFIG=${CONFIG:-configs/pipeline.toml}

stages=(merge clean lid deep_clean near_dedup)
reports=(01_merge_stats 02_clean_stats 03_lid_stats 04_deep_clean_stats 05_near_dedup_stats)
consumed=("" data/merged/merged_so.jsonl data/cleaned/cleaned_so.jsonl data/lid/lid_so.jsonl
  data/deep_clean/deep_clean_so.jsonl)

echo "[$(date '+%F %T')] Pipeline begin (config=$CONFIG, pruning intermediates)" >> "$LOG"
for i in "${!stages[@]}"; do
  stage=${stages[$i]} report=reports/${reports[$i]}.json
  started=$(date +%s)
  echo "[$(date '+%F %T')] STAGE $stage start" >> "$LOG"
  ./target/release/run_pipeline --config "$CONFIG" --stages "$stage" >> "$LOG" 2>&1
  rc=$?
  echo "[$(date '+%F %T')] STAGE $stage end rc=$rc ($(( $(date +%s) - started ))s)" >> "$LOG"
  if [ $rc -ne 0 ]; then
    echo "[$(date '+%F %T')] Pipeline FAILED at $stage; nothing deleted for this stage" >> "$LOG"
    exit $rc
  fi
  if [ ! -s "$report" ] || [ "$(stat -f %m "$report")" -lt "$started" ]; then
    echo "[$(date '+%F %T')] Pipeline FAILED: $report missing or not regenerated" >> "$LOG"
    exit 1
  fi
  prev=${consumed[$i]}
  if [ -n "$prev" ] && [ -f "$prev" ]; then
    echo "[$(date '+%F %T')] PRUNE $prev ($(du -h "$prev" | cut -f1))" >> "$LOG"
    rm -f "$prev"
  fi
done
echo "[$(date '+%F %T')] Pipeline complete" >> "$LOG"
