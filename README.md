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

**Track A is live:** download eleven public Somali datasets, merge, clean, verify language,
and deduplicate into a training-ready corpus. **Track B next:** targeted Somali web collection.

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

All **eleven Track A sources** below are implemented; the v0.2 pipeline has been
**measured end-to-end on the first six** (HPLT, CC100, mC4, OPUS, MADLAD, MT560).
Full-corpus figures for all eleven apply the same stage retention rates to per-source
raw scale (see [docs/SOURCES.md](docs/SOURCES.md)); figures marked with ~ are
projections. v0.1 baseline (six sources, no deep clean): 1.77M docs · 591M words —
see [docs/CLEANING_STRATEGY.md](docs/CLEANING_STRATEGY.md).

### Per-source raw scale (all eleven)

| Source | Class | Raw documents | Notes |
| ------ | ----- | ------------: | ----- |
| HPLT | document | 966,507 | measured download |
| CC100 | document | 396,524 | measured download |
| mC4 | document | 893,012 | measured download |
| MADLAD | document | 200,494 | measured download |
| OPUS | sentence | 14,879 | measured download |
| MT560 | sentence | 161,865 | measured download |
| Wikipedia | document | ~9,000 | HF `20231101.so` |
| XL-Sum | document | ~15,000 | train + validation + test |
| NLLB en–so | sentence | ~1,500,000 | official AllenAI export |
| QuranEnc (Yacob Yusuf) | sentence | 7,373 | 6,236 verses + 1,137 footnotes (measured) |
| Tanzil (Abduh) | sentence | 6,236 | 6,236 ayahs only (no footnotes upstream) |
| **Qur'an subtotal** | | **13,609** | two translations, counted separately |
| **Total (all eleven)** | | **~4.17M** | |

### Projected — full Track A (eleven sources)

| Stage            |     Documents |           Words |    Tokens | Removed this stage |
| ---------------- | ------------: | --------------: | --------: | -----------------: |
| Downloaded (raw) |     ~4,170,000 |           ~860M |    ~1.29B |                  — |
| Merged           |     ~3,690,000 |           ~760M |    ~1.14B |            ~480,000 |
| Cleaned          |     ~3,520,000 |           ~725M |    ~1.09B |            ~170,000 |
| LID verified     |     ~3,340,000 |           ~690M |    ~1.03B |            ~180,000 |
| Deep cleaned     |     ~3,290,000 |           ~680M |    ~1.02B |             ~50,000 |
| **Final**        |     **~3.0M** |       **~555M** |   **~850M** |            ~290,000 |

**Overall (projected):** ~4.17M raw rows → **~3.0M clean documents** · **~555M words**
· **~850M subword tokens** (native 32k BPE, mean 1.53 tokens/word — see
[tokenizer/](tokenizer/)). NLLB adds ~1.4M parallel sentences; both Qur'an
translations (QuranEnc + Tanzil, ~13.6K rows combined) are counted separately;
web-crawl overlap with Wikipedia and XL-Sum is modest. Re-run locally after
downloading all sources to replace projections with measured stats.

### Measured — six-source baseline (2026-07-07)

| Stage            |     Documents |           Words |    Tokens | Removed this stage |
| ---------------- | ------------: | --------------: | --------: | -----------------: |
| Downloaded (raw) |     2,633,281 |           ~835M |    ~1.25B |                  — |
| Merged           |     2,329,800 |           ~738M |    ~1.11B |            303,481 |
| Cleaned          |     2,225,791 |           ~706M |    ~1.06B |            104,009 |
| LID verified     |     2,035,287 |           ~645M |     ~968M |            190,504 |
| Deep cleaned     |     2,003,228 |           ~635M |     ~952M |             32,059 |
| **Final**        | **1,668,080** | **528,853,952** | **~810M** |            335,148 |

**Overall (measured):** 2.63M raw rows → **1.67M clean documents** · **529M words**
· **~810M subword tokens**. Output: `data/final/final_so.jsonl` (~4.0 GB).

### What cleaning removed

| Stage      | Removed | Share of stage input | Main reason                                                        |
| ---------- | ------: | -------------------: | ------------------------------------------------------------------ |
| Merge      | 303,481 |                11.5% | Exact duplicates (MT560 ~68% within-source)                        |
| Clean      | 104,009 |                 4.5% | Too short (&lt;25 words docs / &lt;5 words sentences) or corrupted |
| LID        | 190,504 |                 8.6% | Non-Somali on document-class sources (mC4 highest rate)            |
| Deep clean |  32,059 |                 1.6% | Boilerplate (23,948), segment LID (6,906), too long (1,060)        |
| Near dedup | 335,148 |                16.7% | Near-duplicate web documents (text changed after deep clean)       |

**36.7%** of raw documents did not survive the six-source pipeline (v0.1: 32.6%).
The eleven-source projection assumes similar per-stage rates; sentence-class sources
(NLLB, OPUS, MT560, QuranEnc, Tanzil) skip near-dedup and LID gating. Re-run locally to
reproduce; numbers shift slightly with upstream dataset versions.

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

A corpus-native **32k BPE tokenizer** is trained on the final release corpus
(`data/final/final_so.jsonl`). The trained model ships in-repo; the plain-text
training file is regenerated locally (~3.3 GB).

| Metric                        |              Value |
| ----------------------------- | -----------------: |
| Vocabulary                    |             32,000 |
| Mean tokens/word (native BPE) |               1.53 |
| Median tokens/word            |               1.33 |
| vs BERT-base                  | 2.69 (1.75× worse) |
| vs XLM-RoBERTa                | 1.94 (1.27× worse) |
| Est. corpus tokens (11-src)   |              ~850M |
| Measured (6-src baseline)     |              ~810M |

```bash
cd tokenizer
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt

python prepare_corpus.py --stats   # reads data/final/final_so.jsonl
python train.py                    # writes somali-bpe-tokenizer.json
python test_tokenizer.py           # benchmark; use --sample-size 1668080 for full corpus
```

Artifacts: `somali-bpe-tokenizer.json` (tracked), `benchmark_results.json`,
`tokenizer_stats.json`. Methodology and full results:
[tokenizer/PAPER.md](tokenizer/PAPER.md).

## Quick start

**Requirements:** Rust 1.75+ · ~20 GB free disk for a full build.

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

Scale estimates, overlap, and per-record licensing: [docs/SOURCES.md](docs/SOURCES.md).

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
| [docs/SOURCES.md](docs/SOURCES.md)                     | Source registry and scale estimates             |
| [docs/METADATA_SCHEMA.md](docs/METADATA_SCHEMA.md)     | Record metadata and licensing                   |
| [PLAN.md](PLAN.md)                                     | Vision and two-track strategy                   |
| [ROADMAP.md](ROADMAP.md)                               | Phases and milestones                           |
| [CONTRIBUTING.md](CONTRIBUTING.md)                     | How to contribute                               |
| [tokenizer/PAPER.md](tokenizer/PAPER.md)               | Somali BPE tokenizer methodology and benchmarks |
| [CHANGELOG.md](CHANGELOG.md)                           | Project history                                 |
