# Data Pipeline

How raw Somali text moves from public datasets toward a training-ready corpus.

Full specification: [CLEANING_PLAN.md](CLEANING_PLAN.md). Configuration:
`configs/pipeline.toml`.

## Overview

```text
download → merge + exact dedup → clean → LID → deep clean → near dedup → final
raw/       merged/              cleaned/  lid/   deep_clean/  final/
```

All stages through near-dedup are **implemented** (v0.2 adds `deep_clean` between
LID and near-dedup). Quality filtering (char-n-gram coverage) is deferred until
Wikipedia-so is available as a clean seed (Phase 4).

```bash
cargo build --release
```

## Stage 1 — Download

**Goal:** fetch public Somali datasets and write per-source JSONL under `data/raw/`.

Each downloader writes one JSON object per line:

```json
{"text": "Soomaaliya waa dal ku yaal Geeska Afrika."}
```

MT560 records include a source tag:

```json
{"text": "...", "source": "mt560"}
```

### Downloaders

```bash
./target/release/download_hplt_so   [--limit N] [--no-stream]
./target/release/download_cc100_so  [--limit N] [--no-stream]
./target/release/download_mc4_so    [--limit N] [--no-stream]
./target/release/download_opus_so   [--limit N] [--no-stream]
./target/release/download_madlad_so [--limit N] [--include-noisy]
./target/release/download_mt560_so  [--limit N]
./target/release/download_quran_so  [--limit N] [--concurrency N]
```

| Flag | Description |
|------|-------------|
| `--output <path>` | Override default output path |
| `--limit N` | Stop after N records (smoke tests) |
| `--no-stream` | Download full shard/archive before processing |

Default outputs:

| Source | Path |
|--------|------|
| HPLT | `data/raw/hplt/hplt_so.jsonl` |
| CC100 | `data/raw/cc100/cc100_so.jsonl` |
| mC4 | `data/raw/mc4/mc4_so.jsonl` |
| OPUS | `data/raw/opus/opus_so.jsonl` |
| MADLAD | `data/raw/madlad/madlad_so.jsonl` |
| MT560 | `data/raw/mt560/mt560_so.jsonl` |
| Qur'an | `data/raw/quran/translation.json` + `data/raw/quran/footnotes.json` |

Recommended download order: HPLT → CC100 → mC4 → OPUS → MADLAD → MT560.

## Stage 2 — Merge + exact dedup

**Goal:** combine per-source JSONL into one file with `source` on every line and
streaming exact dedup (first-seen wins).

```bash
./target/release/merge_corpora
./target/release/merge_corpora --config configs/pipeline.toml
./target/release/merge_corpora --raw-dir data/raw --output data/merged/merged_so.jsonl --limit 1000
```

- Source order from `merge_source_order` in `configs/pipeline.toml` (default:
  `mt560 → opus → cc100 → mc4 → madlad → hplt`)
- Missing source files are skipped with a warning
- Output: `data/merged/merged_so.jsonl` (`RawRecord`: `text` + `source`)
- Stats: `reports/01_merge_stats.json`

## Stage 3 — Clean

**Goal:** normalize text, apply per-class length floors, build full `CorpusRecord`
metadata with canonical `content_hash` and `DocId`, post-clean exact recheck.

```bash
./target/release/clean_corpus
./target/release/clean_corpus --input data/merged/merged_so.jsonl --output data/cleaned/cleaned_so.jsonl
./target/release/clean_corpus --config configs/pipeline.toml --limit 1000
```

Cleaning chain (in order): HTML entity decode → mojibake repair (CP1252) → NFC →
control/invisible strip → repeated-char collapse → whitespace normalize → length /
corruption gates.

Length floors (from `configs/pipeline.toml`, benchmarked in
`reports/min_word_threshold_benchmark.md`): **25 words** for document sources
(HPLT, CC100, mC4, MADLAD), **5 words** for sentence sources (OPUS, MT560).

- Output: `data/cleaned/cleaned_so.jsonl`
- Rejects: `data/cleaned/cleaned_so.rejected.jsonl`
- Stats: `reports/02_clean_stats.json`

## Stage 4 — Language identification

**Goal:** verify Somali on document-class sources; tag-only on sentence-class
(OPUS, MT560). Backend chosen by benchmark: **lingua** (see `reports/lid_benchmark.md`).

```bash
./target/release/benchmark_lid   # run before changing LID backend/threshold
./target/release/lid_verify
./target/release/lid_verify --input data/cleaned/cleaned_so.jsonl --output data/lid/lid_so.jsonl
```

- Output: `data/lid/lid_so.jsonl`
- Rejects: `data/lid/lid_so.rejected.jsonl`
- Stats: `reports/03_lid_stats.json`

## Stage 5 — Deep clean (v0.2)

**Goal:** source-aware normalization, markup/contact cleaning, boilerplate removal,
segment-level LID, intra-document dedup, and promoted quality heuristics. Runs on
LID-verified records before near-dedup. Specification:
[CLEANING_STRATEGY.md](CLEANING_STRATEGY.md).

```bash
./target/release/deep_clean
./target/release/deep_clean --input data/lid/lid_so.jsonl --output data/deep_clean/deep_clean_so.jsonl
./target/release/deep_clean --config configs/pipeline.toml --limit 1000
```

- Output: `data/deep_clean/deep_clean_so.jsonl`
- Rejects: `data/deep_clean/deep_clean_so.rejected.jsonl`
- Stats: `reports/04_deep_clean_stats.json`

## Stage 6 — Near dedup

**Goal:** MinHash + LSH near-duplicate removal on **document-class** records only;
sentence-class passes through unchanged. Exact Jaccard verification at τ=0.80;
keep-longest per cluster.

```bash
./target/release/near_dedup
./target/release/near_dedup --input data/deep_clean/deep_clean_so.jsonl --output data/final/final_so.jsonl
```

- Output: `data/final/final_so.jsonl` (release artifact)
- Rejects: `data/final/final_so.rejected.jsonl`
- Stats: `reports/05_near_dedup_stats.json`

## Stage runner

Run the full post-download pipeline in one command:

```bash
./target/release/run_pipeline --config configs/pipeline.toml
./target/release/run_pipeline --stages clean,lid,deep_clean,near_dedup
./target/release/run_pipeline --limit 1000
```

Stages: `merge` → `clean` → `lid` → `deep_clean` → `near_dedup` (invokes sibling
binaries).

## Reports and reject sidecars

Every stage writes **JSON stats** under `reports/` and a companion **Markdown summary**
(`.md` next to each `.json`). Terminal output mirrors the same breakdown: input/kept/
rejected counts, drops by reason, and a per-source table.

### Viewing dropped text

Each stage writes dropped records to a **sidecar JSONL** (full text + reason). When
`run_pipeline` finishes, it prints copy-paste commands and writes `reports/inspect_drops.sh`:

```bash
# All stages
bash reports/inspect_drops.sh

# One stage
bash reports/inspect_drops.sh merge
bash reports/inspect_drops.sh clean
bash reports/inspect_drops.sh lid
bash reports/inspect_drops.sh deep_clean
bash reports/inspect_drops.sh near_dedup
```

| Stage | Dropped text file |
|-------|-------------------|
| merge | `data/merged/merged_so.dropped.jsonl` |
| clean | `data/cleaned/cleaned_so.rejected.jsonl` |
| LID | `data/lid/lid_so.rejected.jsonl` |
| deep clean | `data/deep_clean/deep_clean_so.rejected.jsonl` |
| near dedup | `data/final/final_so.rejected.jsonl` |

Quick preview (clean example):

```bash
jq -r '[.quality.flags[0], .provenance.source, .text] | @tsv' \
  data/cleaned/cleaned_so.rejected.jsonl | head -30
```

Merge drops (exact dedup, before `CorpusRecord`):

```bash
jq -r '[.reason, .source, .text] | @tsv' \
  data/merged/merged_so.dropped.jsonl | head -30
```

| Stage | JSON report | Markdown | Drop sidecar |
|-------|-------------|----------|----------------|
| merge | `reports/01_merge_stats.json` | `reports/01_merge_stats.md` | `data/merged/merged_so.dropped.jsonl` |
| clean | `reports/02_clean_stats.json` | `reports/02_clean_stats.md` | `data/cleaned/cleaned_so.rejected.jsonl` |
| LID | `reports/03_lid_stats.json` | `reports/03_lid_stats.md` | `data/lid/lid_so.rejected.jsonl` |
| deep clean | `reports/04_deep_clean_stats.json` | `reports/04_deep_clean_stats.md` | `data/deep_clean/deep_clean_so.rejected.jsonl` |
| near dedup | `reports/05_near_dedup_stats.json` | `reports/05_near_dedup_stats.md` | `data/final/final_so.rejected.jsonl` |

### Reading drops by reason

**Clean** — `drops_by_reason` in `02_clean_stats.json` (and terminal):

| Flag | Meaning |
|------|---------|
| `too_short` | Below `document_min_words` or `sentence_min_words` for that source class |
| `corrupted` | Replacement-char ratio above `ufffd_reject_ratio` |
| `exact_duplicate_after_clean` | Same normalized text hash as an earlier kept record |

**LID** — document-class sources only (HPLT, CC100, mC4, MADLAD):

| Reason | Meaning |
|--------|---------|
| `not_somali` | Detector returned a non-Somali language |
| `low_lang_score` | Detected Somali but below `min_confidence` |

Sentence-class sources (OPUS, MT560) are tagged only — never rejected at LID.

**Deep clean** — see [CLEANING_STRATEGY.md](CLEANING_STRATEGY.md) for full flag list.
Common reject reasons:

| Reason | Meaning |
|--------|---------|
| `boilerplate` | Too much navigation chrome or site boilerplate removed |
| `not_somali` | Segment-level LID: Somali fraction below threshold |
| `html_remnant` | Script/PHP/scaffolding tags remain after strip |
| `high_symbol_ratio` / `mostly_numbers` | Promoted from review to reject |

**Near dedup** — document-class only:

| Reason | Meaning |
|--------|---------|
| `near_duplicate` | Jaccard similarity ≥ τ against a longer kept document in the same cluster |

### Inspecting rejected records

Reject sidecars are full `CorpusRecord` JSONL — same schema as the kept output, with
`quality.disposition = "rejected"` and `quality.flags` explaining why.

```bash
# Clean rejects: flag, source, first 80 chars of text
jq -r '[.quality.flags[0], .provenance.source, .text[0:80]] | @tsv' \
  data/cleaned/cleaned_so.rejected.jsonl | head -20

# LID rejects: flag, detected lang, snippet
jq -r '[.quality.flags[0], .provenance.lang, .quality.lang_score, .text[0:80]] | @tsv' \
  data/lid/lid_so.rejected.jsonl | head -20

# Near-dedup rejects: canonical id kept instead
jq -r '[.quality.flags[0], .dedup.canonical_id, .text[0:80]] | @tsv' \
  data/final/final_so.rejected.jsonl | head -20

# Count rejects by reason
jq -r '.quality.flags[0]' data/cleaned/cleaned_so.rejected.jsonl | sort | uniq -c
```

Quick report peek without `jq`:

```bash
cat reports/02_clean_stats.md
cat reports/03_lid_stats.md
```

## Record formats

### Raw (merge output) — `RawRecord`

| Field | Type | Description |
|-------|------|-------------|
| `text` | string | Document text (required) |
| `source` | string | Registry key (`hplt`, `cc100`, …) |

### Processed (clean onward) — `CorpusRecord`

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | `{source}:{hash_prefix}` |
| `text` | string | Cleaned UTF-8 text |
| `provenance` | object | Source, `collected_at`, `lang`, optional URL/title/… |
| `license` | string | Per-source SPDX-style identifier |
| `content_hash` | string | SHA-256 hex of normalized cleaned text |
| `dedup` | object | Duplicate metadata |
| `quality` | object | Disposition, flags, lang score |
| `schema_version` | u16 | Currently `1` |

See [METADATA_SCHEMA.md](METADATA_SCHEMA.md) for full field semantics.

## Deferred

- **Quality filter** — char-n-gram coverage against Wikipedia-so seed (Phase 4)
- **Final release packaging** — dataset card, checksums, Hugging Face upload (Phase 6)
