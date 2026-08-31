<h1 align="center">SomNLP-Corpus</h1>

<p align="center">
  <b>A high-quality, scalable, reproducible Somali text corpus for NLP, LLMs, and AI research.</b><br>
  <i>Qoraal Soomaali nadiif ah oo loogu talagalay cilmi-baarista AI iyo NLP-ga.</i>
</p>

<p align="center">
  <b>Rust-first</b> data pipeline · streaming · config-driven · full provenance
</p>

<p align="center">
  <a href="#status">Status</a> ·
  <a href="#what-we-built">What we built</a> ·
  <a href="#corpus-results">Corpus results</a> ·
  <a href="#pipeline">Pipeline</a> ·
  <a href="#tokenizer">Tokenizer</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#sources">Sources</a> ·
  <a href="#docs">Docs</a>
</p>

---

## Status

| Phase                   | Scope                                 | Status  |
| ----------------------- | ------------------------------------- | ------- |
| 1 — Foundation          | Workspace, shared types               | ✅ Done  |
| 2 — Public datasets     | Eleven downloaders + merge            | ✅ Done  |
| 3 — Processing pipeline | Clean → LID → deep clean → near-dedup | ✅ Done  |
| 4 — Collection          | Web scraping & targeted sources       | 🔜 Next  |
| 5 — Release             | Hugging Face packaging                | Planned |

**Track A is live:** eleven public Somali datasets merged, cleaned, and measured end-to-end.

**Current corpus (2026-08-31):** **6.15M documents** · **600M words** · 6.3 GB
(`data/final/final_so.jsonl`). **Track B next:** targeted Somali web collection.

See [ROADMAP.md](ROADMAP.md) and [PLAN.md](PLAN.md).

## What we built

- **Eleven downloaders** — HPLT, CC100, mC4, OPUS, MADLAD, MT560, QuranEnc, Wikipedia, XL-Sum, NLLB, Tanzil
- **Five processing stages** — merge + exact dedup, clean, LID (`lingua`), deep clean (v0.2), near-dedup (MinHash + LSH)
- **`CorpusRecord` metadata** — provenance, content hash, dedup info, quality flags on every kept line
- **Reject sidecars** — full text + reason for every dropped record; inspect with `reports/inspect_drops.sh`
- **Single config** — [`configs/pipeline.toml`](configs/pipeline.toml)

```
SomNLP ── SomNLP-Corpus (this repo) → Translate · NER · QA · Instruct · Sentiment · Bench
```

## Corpus results

All **eleven Track A sources** are implemented and **measured end-to-end** through the
v0.2 pipeline (2026-08-31 run). v0.1 baseline (six sources, no deep clean): 1.77M docs
· 591M words — see [docs/CLEANING_STRATEGY.md](docs/CLEANING_STRATEGY.md). Incremental
measurement notes: [reports/runs/MEASUREMENT.md](reports/runs/MEASUREMENT.md).

### Per-source raw scale (all eleven)

| Source | Class | Raw documents | Notes |
| ------ | ----- | ------------: | ----- |
| HPLT | document | 966,507 | measured download |
| CC100 | document | 396,524 | measured download |
| mC4 | document | 893,012 | measured download |
| MADLAD | document | 200,494 | measured download |
| OPUS | sentence | 14,879 | measured download |
| MT560 | sentence | 161,865 | measured download |
| Wikipedia | document | 9,021 | measured download |
| XL-Sum | document | 7,452 | measured download (train + val + test) |
| NLLB en–so | sentence | 10,229,073 | measured download; official AllenAI export |
| QuranEnc (Yacob Yusuf) | sentence | 7,373 | 6,236 verses + 1,137 footnotes (measured) |
| Tanzil (Abduh) | sentence | 6,236 | 6,236 ayahs only (no footnotes upstream) |
| **Qur'an subtotal** | | **13,609** | two translations, counted separately |
| **Total (all eleven)** | | **12,892,436** | NLLB dominates raw row count |

### Measured — full Track A (eleven sources, 2026-08-31)

| Stage            |     Documents |           Words |    Tokens | Removed this stage |
| ---------------- | ------------: | --------------: | --------: | -----------------: |
| Downloaded (raw) |    12,892,436 |               — |         — |                  — |
| Merged           |     7,087,172 |               — |         — |          5,805,264 |
| Cleaned          |     6,716,987 |               — |         — |            370,185 |
| LID verified     |     6,526,208 |               — |         — |            190,779 |
| Deep cleaned     |     6,493,085 |               — |         — |             33,123 |
| **Final**        | **6,154,601** | **600,085,996** | see tokenizer |            338,484 |

**Overall (measured):** 12.9M raw rows → **6.15M clean documents** · **600M words**.
Output: `data/final/final_so.jsonl` (~6.3 GB). Subword token count is produced by the
[tokenizer pipeline](#tokenizer) (re-run after corpus updates). NLLB contributed 10.2M
raw parallel sentences; 53.8% were within-source duplicates at merge, leaving 4.73M
unique rows (4.46M survive to final).

### Per-source contribution (final documents)

| Source | Raw | Kept at merge | Final | Share of final |
| ------ | --: | ------------: | ----: | -------------: |
| NLLB | 10,229,073 | 4,727,602 | 4,463,021 | 72.5% |
| mC4 | 893,012 | 892,852 | 595,193 | 9.7% |
| HPLT | 966,507 | 798,364 | 576,995 | 9.4% |
| CC100 | 396,524 | 374,721 | 300,664 | 4.9% |
| MADLAD | 200,494 | 200,484 | 133,410 | 2.2% |
| MT560 | 161,865 | 51,083 | 49,195 | 0.8% |
| OPUS | 14,879 | 12,296 | 12,126 | 0.2% |
| QuranEnc | 7,373 | 7,277 | 7,072 | 0.1% |
| XL-Sum | 7,452 | 7,451 | 5,649 | 0.1% |
| Wikipedia | 9,021 | 8,994 | 5,503 | 0.1% |
| Tanzil | 6,236 | 6,048 | 5,773 | 0.1% |
| **Total** | **12,892,436** | **7,087,172** | **6,154,601** | 100% |

NLLB dominates both raw volume and final size. Web crawls (HPLT, mC4, CC100, MADLAD)
lose most documents to near-dedup and LID; sentence-class sources pass through unchanged.

### What cleaning removed (eleven-source run)

| Stage      | Removed | Share of stage input | Main reason                                                        |
| ---------- | ------: | -------------------: | ------------------------------------------------------------------ |
| Merge      | 5,805,264 |                45.0% | Within-source dupes (NLLB 53.8%; MT560 ~68%)                      |
| Clean      |   370,185 |                 5.2% | Too short (&lt;25 words docs / &lt;5 words sentences) or corrupted |
| LID        |   190,779 |                 2.8% | Non-Somali on document-class sources                               |
| Deep clean |    33,123 |                 0.5% | Boilerplate, segment LID, too long                                 |
| Near dedup |   338,484 |                 5.2% | Near-duplicate web documents                                       |

**52.2%** of raw documents did not survive the eleven-source pipeline (mostly NLLB
within-source dedup at merge). Sentence-class sources (NLLB, OPUS, MT560, QuranEnc,
Tanzil) skip near-dedup and LID gating. Re-run locally to reproduce; numbers shift
slightly with upstream dataset versions.

## Pipeline

```text
download → merge + exact dedup → clean → LID → deep clean → near dedup → final
raw/       merged/              cleaned/  lid/   deep_clean/  final/
```

| Stage       | Binary                               | Output                                |
| ----------- | ------------------------------------ | ------------------------------------- |
| Download    | `download_*_so` / `download_quran_*` | `data/raw/<source>/`                  |
| Merge       | `merge_corpora`                      | `data/merged/merged_so.jsonl`         |
| Clean       | `clean_corpus`                       | `data/cleaned/cleaned_so.jsonl`       |
| Language ID | `lid_verify`                         | `data/lid/lid_so.jsonl`               |
| Deep clean  | `deep_clean`                         | `data/deep_clean/deep_clean_so.jsonl` |
| Near dedup  | `near_dedup`                         | `data/final/final_so.jsonl`           |
| All stages  | `run_pipeline`                       | chains the above                      |

| Source class | Sources                                     | Min words | LID                  | Near dedup    |
| ------------ | ------------------------------------------- | --------: | -------------------- | ------------- |
| Document     | HPLT, CC100, mC4, MADLAD, Wikipedia, XL-Sum |        25 | `lingua` gate @ 0.50 | MinHash + LSH |
| Sentence     | OPUS, MT560, QuranEnc, NLLB, Tanzil         |         5 | tag-only             | exact only    |

Full commands and drop inspection: [docs/DATA_PIPELINE.md](docs/DATA_PIPELINE.md) ·
specification: [docs/CLEANING_PLAN.md](docs/CLEANING_PLAN.md).

## Tokenizer

Four Python scripts train and evaluate corpus-native BPE tokenizers on
`data/final/final_so.jsonl`:

```text
final_so.jsonl  →  prepare_corpus.py  →  somali_raw_corpus.txt   (training text)
                                      →  eval_holdout.jsonl      (held-out split)
                                              ↓
                                         train.py  →  somali-bpe-v2.json
                                              ↓
                                       benchmark.py  →  benchmark_results.json
                                              ↓
                                       export_hf.py  →  v2/  (AutoTokenizer-loadable)
```

1. **`prepare_corpus.py`** — streams JSONL, extracts each record's `text`, applies NFC
   normalization, and splits documents into training text (~6 GB) and a deterministic
   held-out evaluation split (`blake2b-8(id) mod 1000 < 8`, ~0.8%). `--stats` writes
   per-source document/word counts and a corpus fingerprint to `tokenizer_stats.json`.
2. **`train.py`** — trains a ByteLevel BPE via streaming iterator; the corpus is never
   loaded into RAM. Refuses to run if the training text no longer matches the release
   corpus. `--sweep` trains several vocabulary sizes; `--derive-from` produces smaller
   sizes by truncating a larger run's merge list (exact for BPE, and far cheaper).
3. **`benchmark.py`** — scores every tokenizer on the same held-out documents in one
   streaming pass: tokens/word, bytes/token, round-trip fidelity, unknown-token rate, and
   a per-source breakdown, against optional BERT-base and XLM-RoBERTa baselines.
4. **`export_hf.py`** — promotes a chosen candidate to `somali-bpe-v2.json` and writes a
   `transformers`-loadable `v2/` directory.

### v1 and v2

**v1** (`somali-bpe-tokenizer.json`, 32k) is retained unchanged so its published figures
stay reproducible. It has two defects that make it unsuitable for a generative model:
`decode()` does not restore whitespace (its `BPEDecoder` expects an `</w>` suffix the
trainer never emitted), and out-of-alphabet characters collapse to `<unk>`.

**v2** (`somali-bpe-v2.json`) is a ByteLevel BPE. Decoding inverts encoding exactly, and
an unknown token is unreachable because all 256 bytes seed the alphabet. Both properties
are enforced by the test suite rather than merely observed.

| Metric | v1 (32k, whitespace) | **v2 (48k, ByteLevel)** |
| ------ | -------------------: | ----------------------: |
| Mean tokens/word | 1.3936 | **1.3528** |
| Median tokens/word | 1.3333 | **1.2857** |
| P95 tokens/word | 1.9000 | **1.8571** |
| Bytes/token | 4.734 | **4.807** |
| Round-trip fidelity | 0.000 | **1.000** |
| Docs emitting `<unk>` | 4 | **0** |
| vs BERT-base (2.6291) | — | **1.94× better** |
| vs XLM-RoBERTa (1.8233) | — | **1.35× better** |

Measured on a held-out split of **49,424 documents** excluded from v2 training
(`benchmark_results.json`). v1 fails to reconstruct whitespace on all 49,424 of them.
These figures are not comparable with the 1.53 published on 2026-07-07: that benchmark
scored *lines* of the training text, and 21% of documents span several lines.

Corpus totals: **6,154,594 documents · 600,085,996 words · 826M v2 subword tokens**.

```bash
cd tokenizer
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt

pytest                             # property + unit tests, ~4s, no network
python prepare_corpus.py --stats   # writes training text + eval holdout
python train.py                    # trains at the default vocabulary size
python benchmark.py                # scores v1 and v2 on the holdout
```

Vocabulary sweep, then promote the winner:

```bash
python train.py --vocab-size 65536 --output sweep/somali-bpe-v2-65536.json
python train.py --sweep 16384,32000,48000 --derive-from sweep/somali-bpe-v2-65536.json
python benchmark.py --sweep-dir sweep
python export_hf.py sweep/somali-bpe-v2-<chosen>.json
```

Tracked artifacts: `somali-bpe-tokenizer.json` (v1), `somali-bpe-v2.json` (v2), `v2/`,
`benchmark_results.json`, `tokenizer_stats.json`. The training text, eval holdout, and
`sweep/` are regenerable and gitignored. Methodology and full results:
[tokenizer/PAPER.md](tokenizer/PAPER.md).

## Quick start

**Requirements:** Rust 1.75+ · ~40 GB free disk for a full eleven-source build (NLLB is large).

```bash
cargo build --release
```

### Smoke test (~100 records)

```bash
./target/release/download_hplt_so --limit 100
./target/release/run_pipeline --stages merge,clean,lid,deep_clean,near_dedup --limit 100
```

### Full corpus build

```bash
./target/release/download_hplt_so
./target/release/download_cc100_so
./target/release/download_mc4_so
./target/release/download_opus_so
./target/release/download_madlad_so
./target/release/download_mt560_so
./target/release/download_quran_so
./target/release/download_wikipedia_so
./target/release/download_xlsum_so
./target/release/download_nllb_so
./target/release/download_quran_tanzil

./target/release/run_pipeline --config configs/pipeline.toml
```

Some Hugging Face datasets need authentication:

```bash
export HF_TOKEN=hf_...   # or HUGGING_FACE_HUB_TOKEN
```

### Inspect drops

```bash
bash reports/inspect_drops.sh          # all stages
bash reports/inspect_drops.sh clean    # one stage
```

Per-run stats live in `reports/` (gitignored). Corpus artifacts in `data/` (gitignored).

### Development

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Sources

| Tool                    | Dataset                                                                                                | License      |
| ----------------------- | ------------------------------------------------------------------------------------------------------ | ------------ |
| `download_hplt_so`      | [HPLT2.0 cleaned](https://huggingface.co/datasets/HPLT/HPLT2.0_cleaned) (`som_Latn`)                   | CC0-1.0      |
| `download_cc100_so`     | [CC-100 Somali](https://data.statmt.org/cc-100/so.txt.xz)                                              | CC-BY-SA-4.0 |
| `download_mc4_so`       | [allenai/c4](https://huggingface.co/datasets/allenai/c4) (`so`)                                        | ODC-BY       |
| `download_opus_so`      | [OPUS ParaCrawl](https://huggingface.co/datasets/Helsinki-NLP/opus_paracrawl) (`en-so`)                | CC0-1.0      |
| `download_madlad_so`    | [MADLAD-400](https://huggingface.co/datasets/allenai/MADLAD-400) (`so`)                                | ODC-BY       |
| `download_mt560_so`     | [MT560 en–so pairs](https://huggingface.co/datasets/michsethowusu/english-somali_sentence-pairs_mt560) | CC-BY-4.0    |
| `download_quran_so`     | [QuranEnc Somali (Yacob Yusuf)](https://quranenc.com/api/v1/translation/sura/somali_yacob/1)           | see source   |
| `download_wikipedia_so` | [Somali Wikipedia](https://huggingface.co/datasets/wikimedia/wikipedia) (`20231101.so`)                | CC-BY-SA-4.0 |
| `download_xlsum_so`     | [XL-Sum Somali](https://huggingface.co/datasets/csebuetnlp/xlsum) (`somali`)                           | CC-BY-4.0    |
| `download_nllb_so`      | [NLLB English–Somali](https://storage.googleapis.com/allennlp-data-bucket/nllb/eng_Latn-som_Latn.gz)   | ODC-BY       |
| `download_quran_tanzil` | [Tanzil Qur'an Somali](https://tanzil.net/trans/?transID=so.abduh) (`so.abduh`)                        | see source   |

Scale, overlap, and per-record licensing: [docs/SOURCES.md](docs/SOURCES.md).
Full eleven-source pipeline stats: [reports/runs/MEASUREMENT.md](reports/runs/MEASUREMENT.md).

> **Licensing:** no single corpus license — each `CorpusRecord` carries its upstream
> `license` field. See [docs/METADATA_SCHEMA.md](docs/METADATA_SCHEMA.md).

## Record format

```json
{
  "id": "hplt:a3f8c2…",
  "text": "Soomaaliya waa dal ku yaal Geeska Afrika.",
  "provenance": { "source": "hplt", "lang": "so", "collected_at": "…" },
  "license": "CC0-1.0",
  "content_hash": "sha256:…",
  "quality": { "disposition": "kept", "flags": [] },
  "schema_version": 1
}
```

## Project layout

```text
somnlp/
├── configs/pipeline.toml       # merge order, clean/LID/dedup knobs
├── crates/
│   ├── common/                 # record types, hashing, source registry
│   ├── corpus-tools/           # downloaders + merge
│   └── corpus-pipeline/        # clean, LID, deep clean, near-dedup, run_pipeline
├── docs/                       # architecture, schema, pipeline specs
├── tokenizer/                  # Somali BPE training pipeline + trained model
├── reports/                    # per-run stats (gitignored)
└── data/                       # corpus artifacts (gitignored)
```

Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Docs

| Doc                                                    | Description                                     |
| ------------------------------------------------------ | ----------------------------------------------- |
| [docs/DATA_PIPELINE.md](docs/DATA_PIPELINE.md)         | Stage commands, data flow, inspecting drops     |
| [docs/CLEANING_PLAN.md](docs/CLEANING_PLAN.md)         | Phase 3 cleaning, LID, and dedup specification  |
| [docs/CLEANING_STRATEGY.md](docs/CLEANING_STRATEGY.md) | v0.2 deep-clean audit and strategy              |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)           | Workspace layout and crate design               |
| [docs/SOURCES.md](docs/SOURCES.md)                     | Source registry and measured counts             |
| [docs/METADATA_SCHEMA.md](docs/METADATA_SCHEMA.md)     | Record metadata and licensing                   |
| [PLAN.md](PLAN.md)                                     | Vision and two-track strategy                   |
| [ROADMAP.md](ROADMAP.md)                               | Phases and milestones                           |
| [CONTRIBUTING.md](CONTRIBUTING.md)                     | How to contribute                               |
| [tokenizer/PAPER.md](tokenizer/PAPER.md)               | Somali BPE tokenizer methodology and benchmarks |
| [CHANGELOG.md](CHANGELOG.md)                           | Project history                                 |
