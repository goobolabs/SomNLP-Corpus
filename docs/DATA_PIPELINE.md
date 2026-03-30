# Data Pipeline

How raw Somali text moves from public datasets toward a training-ready corpus.

## Overview

```text
public datasets ──▶ download ──▶ merge ──▶ [clean] ──▶ [dedup] ──▶ [filter] ──▶ [final]
                    raw/        merged/     cleaned/    dedup/      filtered/    final/
```

Stages in **bold** are implemented today. Bracketed stages are planned.

## Stage 1 — Download (implemented)

**Goal:** fetch public Somali datasets and write per-source JSONL under `data/raw/`.

Each downloader streams or downloads source shards, extracts Somali text, and
writes one JSON object per line:

```json
{"text": "Soomaaliya waa dal ku yaal Geeska Afrika."}
```

MT560 records include a source tag:

```json
{"text": "...", "source": "mt560"}
```

### Available downloaders

```bash
cargo build --release

./target/release/download_hplt_so   [--limit N] [--no-stream]
./target/release/download_cc100_so  [--limit N] [--no-stream]
./target/release/download_mc4_so    [--limit N] [--no-stream]
./target/release/download_opus_so   [--limit N] [--no-stream]
./target/release/download_madlad_so [--limit N] [--include-noisy]
./target/release/download_mt560_so  [--limit N]
```

Common flags:

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

Recommended download order: HPLT → CC100 → mC4 → OPUS → MADLAD → MT560.

## Stage 2 — Merge (implemented)

**Goal:** combine per-source JSONL files into one merged corpus.

```bash
./target/release/merge_corpora
./target/release/merge_corpora --sources hplt cc100 mc4
./target/release/merge_corpora --raw-dir data/raw --output data/merged/merged_so.jsonl
```

- Default sources: `cc100`, `hplt`, `mc4`, `opus`, `madlad`, `mt560`
- Missing source files are skipped with a warning
- Output: `data/merged/merged_so.jsonl`
- Prints per-source document counts and totals

## Stage 3 — Clean (planned)

Normalize text: fix encoding, strip HTML remnants, collapse whitespace, apply
Somali-aware rules. Output to `data/cleaned/`.

## Stage 4 — Deduplicate (planned)

Remove exact and near-duplicate documents within and across sources. Output to
`data/deduplicated/`.

## Stage 5 — Language filter (planned)

Identify and drop non-Somali text. Output to `data/filtered/`.

## Stage 6 — Final export (planned)

Apply quality gates, produce release-ready JSONL/Parquet, and generate corpus
statistics. Output to `data/final/`.

## Record format

The current `Document` type in `crates/common` matches downloader output:

| Field | Type | Description |
|-------|------|-------------|
| `text` | string | Document text (required) |
| `source` | string, optional | Dataset identifier when present |

The schema will gain provenance, language scores, dedup metadata, and quality
flags as later stages are implemented.
