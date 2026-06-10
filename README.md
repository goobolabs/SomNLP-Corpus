# SomNLP-Corpus

A high-quality, reproducible Somali text corpus for NLP and LLM research.

Rust-first tooling for downloading public Somali datasets, merging them into a
single raw corpus, and (over time) cleaning and preparing text for training.

## Status

Early development. **Download, merge, clean, language identification, and
near-dedup** are implemented in Rust (`corpus-tools` + `corpus-pipeline`).
Quality filtering and web collection are planned next.

## Quick start

Requires Rust 1.75+.

```bash
cargo build --release
```

Smoke-test a downloader (first 100 records):

```bash
./target/release/download_hplt_so --limit 100
./target/release/download_cc100_so --limit 100
```

Merge downloaded sources:

```bash
./target/release/merge_corpora
./target/release/merge_corpora --sources hplt cc100
```

Run the full processing pipeline (merge → clean → LID → near-dedup):

```bash
./target/release/run_pipeline --config configs/pipeline.toml
./target/release/run_pipeline --stages clean,lid,near_dedup --limit 1000
```

Full usage and dataset details: see [docs/DATA_PIPELINE.md](docs/DATA_PIPELINE.md).

## Public datasets (implemented)

| Tool | Source | Output |
|------|--------|--------|
| `download_hplt_so` | [HPLT2.0 cleaned](https://huggingface.co/datasets/HPLT/HPLT2.0_cleaned) (`som_Latn`) | `data/raw/hplt/hplt_so.jsonl` |
| `download_cc100_so` | [CC-100 Somali](https://data.statmt.org/cc-100/so.txt.xz) | `data/raw/cc100/cc100_so.jsonl` |
| `download_mc4_so` | [allenai/c4](https://huggingface.co/datasets/allenai/c4) (`so`) | `data/raw/mc4/mc4_so.jsonl` |
| `download_opus_so` | [opus_paracrawl](https://huggingface.co/datasets/Helsinki-NLP/opus_paracrawl) (`en-so`) | `data/raw/opus/opus_so.jsonl` |
| `download_madlad_so` | [MADLAD-400](https://huggingface.co/datasets/allenai/MADLAD-400) (`so`) | `data/raw/madlad/madlad_so.jsonl` |
| `download_mt560_so` | [MT560 en–so pairs](https://huggingface.co/datasets/michsethowusu/english-somali_sentence-pairs_mt560) | `data/raw/mt560/mt560_so.jsonl` |
| `merge_corpora` | — | `data/merged/merged_so.jsonl` |

## Project layout

```text
somnlp/
├── crates/
│   ├── common/         # shared record types
│   └── corpus-tools/   # dataset downloaders + merge
├── docs/               # architecture and pipeline docs
├── data/               # generated corpus artifacts (not tracked in git)
└── Cargo.toml          # workspace manifest
```

## Documentation

| Doc | Description |
|-----|-------------|
| [PLAN.md](PLAN.md) | Vision, two-track strategy, corpus targets |
| [ROADMAP.md](ROADMAP.md) | Phases and milestones |
| [docs/SOURCES.md](docs/SOURCES.md) | Source registry and token estimates |
| [docs/METADATA_SCHEMA.md](docs/METADATA_SCHEMA.md) | Record metadata and licensing model |
| [docs/MERGE_SEMANTICS.md](docs/MERGE_SEMANTICS.md) | Merge behavior and provenance |
| [docs/QUALITY_METADATA.md](docs/QUALITY_METADATA.md) | Dedup and quality metadata |
| [docs/ID_STRATEGY.md](docs/ID_STRATEGY.md) | Document IDs and content hashes |
| [docs/SCHEMA_REVIEW.md](docs/SCHEMA_REVIEW.md) | Schema design review |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Workspace layout and crate responsibilities |
| [docs/DATA_PIPELINE.md](docs/DATA_PIPELINE.md) | Pipeline stages, data flow, and usage |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute |
| [CHANGELOG.md](CHANGELOG.md) | Project history |
