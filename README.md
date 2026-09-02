<h1 align="center">SomNLP-Corpus</h1>

<p align="center">
  <strong>A reproducible Somali text corpus and data-processing pipeline for NLP and language-model research.</strong><br>
  <em>Qoraal Soomaali nadiif ah oo loogu talagalay cilmi-baarista AI iyo NLP-ga.</em>
</p>

<p align="center">
  Rust-first · streaming · configuration-driven · source-level provenance
</p>

SomNLP-Corpus downloads public Somali datasets, normalizes and filters their text,
removes exact and near duplicates, and emits training-ready JSONL. The repository also
contains a corpus-trained ByteLevel BPE tokenizer.

## Current release

The latest measured build completed on **2026-09-02** and combines **13 public sources**:

| Final documents | Words | v2 subword tokens | JSONL size |
| --------------: | ----: | ----------------: | ---------: |
| 7,352,961 | 665,985,672 | 911,824,557 | 7.2 GB |

The release artifact is `data/final/final_so.jsonl`. Corpus data is not stored in Git;
build it with the pipeline below. Trained tokenizer artifacts and their benchmark summary
are tracked for reproducibility.

Public-dataset ingestion and the processing pipeline are complete. Targeted web
collection and Hugging Face packaging are in progress. See [ROADMAP.md](ROADMAP.md) for
milestones and [CHANGELOG.md](CHANGELOG.md) for project history.

## What is included

- Downloaders for HPLT, CC100, mC4, OPUS, MADLAD, MT560, QuranEnc, Tanzil,
  Wikipedia, XL-Sum, NLLB, Glot500, and Somali Web Corpus
- Streaming merge and exact deduplication
- Text normalization, language identification, source-aware deep cleaning, and
  MinHash/LSH near-deduplication
- Per-record provenance, source license, content hash, deduplication metadata, and
  quality metadata
- Reject sidecars and per-stage statistics for auditing removed records
- A 48k-vocabulary Somali ByteLevel BPE tokenizer and reproducible training scripts

All pipeline settings live in [`configs/pipeline.toml`](configs/pipeline.toml).

## Pipeline and measured retention

```text
download → merge + exact dedup → clean → LID → deep clean → near dedup → final
raw/       merged/              cleaned/  lid/   deep_clean/  final/
```

| Stage | Documents | Removed at stage |
| ----- | --------: | ---------------: |
| Downloaded | 17,025,862 | — |
| Merged | 10,790,439 | 6,235,423 |
| Cleaned | 8,104,479 | 2,685,960 |
| LID verified | 7,791,520 | 312,959 |
| Deep cleaned | 7,757,223 | 34,297 |
| **Final** | **7,352,961** | **404,262** |

These figures describe the 2026-09-02 run (13 sources) and may change when upstream
datasets change. Run 12 (twelve sources, 2026-09-01): 7.24M final · 662M words · 906M
tokens. Each local run writes detailed stage statistics under `reports/`.

Document-class sources use a 25-word minimum, Somali language-ID gating, and near
deduplication. Sentence-class sources use a 5-word minimum and exact deduplication; their
language score is recorded but not used as a rejection gate. The complete behavior and
commands are documented in [`docs/DATA_PIPELINE.md`](docs/DATA_PIPELINE.md).

## Quick start

### Requirements

- Rust 1.75 or newer
- Approximately 45 GB of free disk space for a full build
- Network access to the upstream datasets
- A Hugging Face token for sources that require authentication (`HF_TOKEN` or
  `HUGGING_FACE_HUB_TOKEN`)

Build all tools:

```bash
cargo build --release
```

Run a small end-to-end smoke test:

```bash
./target/release/download_hplt_so --limit 100
./target/release/run_pipeline \
  --stages merge,clean,lid,deep_clean,near_dedup \
  --limit 100
```

The smoke test writes under `data/`. Move or remove that test data before starting a full
build if you want a clean run.

### Build the full corpus

Run the 13 downloaders:

```bash
./target/release/download_hplt_so
./target/release/download_cc100_so
./target/release/download_mc4_so
./target/release/download_opus_so
./target/release/download_madlad_so
./target/release/download_mt560_so
./target/release/download_quran_so
./target/release/download_quran_tanzil
./target/release/download_wikipedia_so
./target/release/download_xlsum_so
./target/release/download_nllb_so
./target/release/download_glot_so
./target/release/download_somali_web_corpus_so
```

Then run every processing stage:

```bash
./target/release/run_pipeline --config configs/pipeline.toml
```

Generated corpus files and local run reports are ignored by Git. The pipeline is
streaming, but the largest stages still require substantial disk space and runtime.

## Sources and licensing

| Source | Data class | Upstream license |
| ------ | ---------- | ---------------- |
| HPLT 2.0 (`som_Latn`) | document | CC0-1.0 |
| CC-100 Somali | document | CC-BY-SA-4.0 |
| mC4 Somali | document | ODC-BY |
| MADLAD-400 Somali | document | ODC-BY |
| Glot500 (`som_Latn`) | document | source-specific |
| Somali Web Corpus V1 | document | MIT |
| Somali Wikipedia | document | CC-BY-SA-4.0 |
| XL-Sum Somali | document | CC-BY-4.0 |
| OPUS ParaCrawl (`en-so`) | sentence | CC0-1.0 |
| MT560 (`en-so`) | sentence | CC-BY-4.0 |
| NLLB (`eng_Latn-som_Latn`) | sentence | ODC-BY |
| QuranEnc Somali (Yacob Yusuf) | sentence | source-specific |
| Tanzil Somali (Abduh) | sentence | source-specific |

> **There is no single corpus-wide license.** Every processed record carries its
> upstream license. Users are responsible for following the terms of each source when
> redistributing or using the combined corpus.

Dataset URLs, access methods, measured scale, and detailed licensing notes are maintained
in [`docs/SOURCES.md`](docs/SOURCES.md). Record-level licensing is described in
[`docs/METADATA_SCHEMA.md`](docs/METADATA_SCHEMA.md).

## Record format

Processed records are newline-delimited JSON with this shape (values abbreviated):

```json
{
  "id": "hplt:a3f8c2…",
  "text": "Soomaaliya waa dal ku yaal Geeska Afrika.",
  "provenance": {
    "source": "hplt",
    "collected_at": "2026-09-01T00:00:00Z",
    "lang": "so"
  },
  "license": "CC0-1.0",
  "content_hash": "a3f8c2…",
  "dedup": { "is_duplicate": false, "content_hash": "a3f8c2…" },
  "quality": { "disposition": "kept", "flags": [] },
  "schema_version": 1
}
```

See [`docs/METADATA_SCHEMA.md`](docs/METADATA_SCHEMA.md) for optional provenance,
quality, and metadata fields.

## Tokenizer

The repository includes two trained tokenizer artifacts:

- **v1:** a retained 32k whitespace BPE baseline
- **v2:** the recommended 48k ByteLevel BPE with exact encode/decode round trips and
  byte-level coverage

| Metric | v1 (32k) | **v2 (48k)** |
| ------ | --------: | -----------: |
| Mean tokens/word | 1.3936 | **1.3528** |
| P95 tokens/word | 1.9000 | **1.8571** |
| Bytes/token | 4.734 | **4.807** |
| Round-trip fidelity | 0.000 | **1.000** |
| Documents emitting `<unk>` | 4 | **0** |

The benchmark used 49,424 held-out documents from the earlier 11-source corpus. The
912M-token release total was measured on the 13-source corpus with v2 (2026-09-02); it is
not a retrained 13-source benchmark.

To run the tokenizer tests and reproduce its preparation, training, and benchmark steps:

```bash
cd tokenizer
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt

pytest
python prepare_corpus.py --stats
python train.py
python benchmark.py
```

The training corpus and evaluation split are generated locally and ignored by Git. See
[`tokenizer/PAPER.md`](tokenizer/PAPER.md) for methodology, vocabulary-size experiments,
baseline comparisons, and Hugging Face export instructions.

## Development

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

Contribution workflow and code standards are in
[`CONTRIBUTING.md`](CONTRIBUTING.md).

## Repository layout

```text
SomNLP-Corpus/
├── configs/pipeline.toml       # pipeline settings and source order
├── crates/
│   ├── common/                 # record types, hashing, source registry
│   ├── corpus-tools/           # downloaders and merge logic
│   └── corpus-pipeline/        # cleaning, LID, dedup, stage runner
├── docs/                       # specifications and technical documentation
├── reports/                    # selected measurements and local run output
├── tokenizer/                  # tokenizer scripts, tests, and trained artifacts
└── data/                       # generated corpus data; not tracked
```

Start with [`docs/DATA_PIPELINE.md`](docs/DATA_PIPELINE.md) to operate the pipeline,
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) to understand the codebase, and
[`docs/SOURCES.md`](docs/SOURCES.md) to evaluate the input data.
