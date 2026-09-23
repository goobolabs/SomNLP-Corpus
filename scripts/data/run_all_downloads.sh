#!/usr/bin/env bash
# Run target/release/download_* concurrently (PARALLEL, default: all at once), retry each
# failure once, and record rows + output size in reports/full_download_results.tsv.
# Usage: run_all_downloads.sh [downloader ...]   (default: every download_* binary)
set -u
cd "$(dirname "$0")/../.."
LOG=reports/full_download.log
RES=reports/full_download_results.tsv
mkdir -p reports/download_logs
[ -f "$RES" ] || printf "downloader\tstatus\tattempts\tseconds\trows\tbytes\toutput_dir\terror\n" > "$RES"

outdir() {
  case "$1" in
    download_quran_so) echo data/raw/quran ;;
    download_quran_tanzil) echo data/raw/quran-tanzil ;;
    download_somali_web_corpus_so) echo data/raw/somali-web-corpus ;;
    download_somali_dataset_so) echo data/raw/somali-dataset ;;
    download_somali_tinystories_so) echo data/raw/somali-tinystories ;;
    download_fineweb2_so) echo data/raw/fineweb-2 ;;
    *) local s=${1#download_}; echo "data/raw/${s%_so}" ;;
  esac
}

count_rows() {  # every raw file is line-delimited JSON (Qur'an files too, despite .json)
  cat "$1"/*.json "$1"/*.jsonl 2>/dev/null | grep -c .
}

run_one() {
  local bin=$1 dir attempt=0 rc=1 start end err=""
  dir=$(outdir "$bin")
  start=$(date +%s)
  while [ $attempt -lt 2 ] && [ $rc -ne 0 ]; do
    attempt=$((attempt + 1))
    echo "[$(date '+%F %T')] START $bin (attempt $attempt)" >> "$LOG"
    ./target/release/"$bin" > "reports/download_logs/$bin.attempt$attempt.log" 2>&1
    rc=$?
    echo "[$(date '+%F %T')] END $bin rc=$rc (attempt $attempt)" >> "$LOG"
    grep -v $'\r' "reports/download_logs/$bin.attempt$attempt.log" | tail -25 | sed "s/^/  [$bin] /" >> "$LOG"
  done
  end=$(date +%s)
  if [ $rc -eq 0 ]; then
    local rows bytes
    rows=$(count_rows "$dir" 2>/dev/null || echo "?")
    bytes=$(du -sk "$dir" | awk '{print $1*1024}')
    printf "%s\tok\t%s\t%s\t%s\t%s\t%s\t\n" "$bin" "$attempt" "$((end - start))" "$rows" "$bytes" "$dir" >> "$RES"
  else
    err=$(grep -v $'\r' "reports/download_logs/$bin.attempt$attempt.log" | grep -i "error" | tail -1 | tr '\t' ' ')
    printf "%s\tFAILED\t%s\t%s\t\t\t%s\t%s\n" "$bin" "$attempt" "$((end - start))" "$dir" "$err" >> "$RES"
  fi
}
export -f run_one outdir count_rows
export LOG RES

if [ $# -gt 0 ]; then bins=$(printf "%s\n" "$@"); else bins=$(ls target/release/ | grep '^download_' | grep -v '\.d$'); fi
PARALLEL=${PARALLEL:-$(echo "$bins" | wc -l)}
echo "[$(date '+%F %T')] Phase 1 begin: $(echo $bins | wc -w | tr -d ' ') downloaders, parallel=$PARALLEL" >> "$LOG"
echo "$bins" | xargs -P "$PARALLEL" -I{} bash -c 'run_one {}'
echo "[$(date '+%F %T')] Phase 1 complete" >> "$LOG"
