# Changelog

All notable changes to SomNLP-Corpus are documented here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **Eleven-source measured run (2026-08-31)** — full Track A pipeline on all downloaders:
  12.9M raw → 6,154,594 final documents · 600M words · **826M v2 subword tokens** (measured)
- Incremental measurement log: [reports/runs/MEASUREMENT.md](reports/runs/MEASUREMENT.md)
- **`merge_source_order`** in `configs/pipeline.toml` extended to all eleven sources
- **Somali BPE tokenizer pipeline** (`tokenizer/`) — prepare, train, benchmark, and export
  scripts using Hugging Face `tokenizers` on the final release corpus
- **v2 tokenizer** (`tokenizer/somali-bpe-v2.json`, 48k ByteLevel BPE) trained on the full
  eleven-source corpus, plus a `transformers`-loadable export at `tokenizer/v2/`
- Deterministic held-out evaluation split (49,424 documents, `blake2b-8(id) mod 1000 < 8`)
- Per-source composition stats in `tokenizer_stats.json` — document share and word share
  diverge sharply (NLLB is 72.5% of documents but 11.3% of words)
- Property test suite for the tokenizer (round-trip losslessness, zero-`<unk>` invariant,
  vocabulary-truncation equivalence) plus `pyproject.toml` with ruff and pytest config
- Technical note: [tokenizer/PAPER.md](tokenizer/PAPER.md)

### Fixed

- **Tokenizer decode was destroying whitespace.** v1's `BPEDecoder` was configured to strip
  an `</w>` suffix its trainer never emitted, so `decode(encode(x))` concatenated all tokens:
  `"Soomaaliya waa dal"` → `"Soomaaliyawaadal"`. It fails on **all 49,424** held-out
  documents. v2's ByteLevel decoder round-trips exactly (fidelity 1.000).
- **Unknown-token loss.** v1 mapped out-of-alphabet characters to `<unk>` (4 held-out
  documents affected). v2 seeds all 256 bytes, making `<unk>` unreachable.
- **Benchmark memory.** The reservoir sampler held every sampled document in memory at once
  (an estimated 10–14 GB at full corpus size); it now streams and retains only per-document
  ratios.
- **Silent training on a stale corpus.** `train.py` checked only that the training text
  existed. It now compares a corpus fingerprint recorded by `prepare_corpus.py` and refuses
  to proceed on a mismatch unless `--allow-stale` is passed.
- **`somali_raw_corpus.txt` was never one document per line.** 21% of documents contain
  internal newlines, so v1's published "per-document" ratios were largely per-paragraph.
  The evaluation split is now JSONL, making the unit unambiguous.

### Changed

- v1 benchmark figures superseded. Under the corrected document-level protocol on the
  eleven-source corpus, v1 measures 1.3936 mean tokens/word and v2 measures
  **1.3528**, against 2.6291 (BERT-base) and 1.8233 (XLM-RoBERTa).
  The previously published 1.53 is not comparable — it scored lines, not documents.
- `tokenizer/test_tokenizer.py` renamed to `tokenizer/benchmark.py`; it is a benchmark CLI,
  and the old name caused pytest to collect a file containing no tests.
- `tokenizer/somali-bpe-tokenizer.json` (v1) retained unchanged so its figures stay
  reproducible.

### Planned

- Hugging Face release packaging (`v0.2-clean`)
- Wikipedia and Somali web collectors
- Books, subtitles, OCR, and community contribution intake

## [0.2.0] — 2026-07-07

### Added

- **v0.2 pipeline topology** — new `deep_clean` stage between LID and near-dedup:
  `merge → clean → lid → deep_clean → near_dedup → final`
- `deep_clean` binary and `[deep_clean]` config section (see `docs/CLEANING_STRATEGY.md`)
- Deep-clean sub-stages: source-aware normalize, HTML/contact masking, boilerplate
  removal, segment-level LID, intra-doc dedup, quality heuristics v2
- Output paths: `data/deep_clean/deep_clean_so.jsonl`; release remains `data/final/final_so.jsonl`
- Stage reports renumbered: `reports/04_deep_clean_stats.json`, `reports/05_near_dedup_stats.json`
- `QualityFlag::Boilerplate` for deep-clean rejects
- Post-run cleaning audit (`reports/06_cleaning_audit.md`)

### Changed

- `near_dedup` default input: `data/deep_clean/deep_clean_so.jsonl` (was `data/lid/lid_so.jsonl`)
- `run_pipeline` runs five post-merge stages including `deep_clean`
- MADLAD export unescape and OPUS HTML escape strip (P0 export fixes)

### Corpus results (full 6-source run)

| Stage | Documents | Removed |
|-------|----------:|--------:|
| Raw (merged input) | 2,633,281 | — |
| Merged | 2,329,800 | 303,481 |
| Cleaned | 2,225,791 | 104,009 |
| LID verified | 2,035,287 | 190,504 |
| Deep cleaned | 2,003,228 | 32,059 |
| **Final** | **1,668,080** | 335,148 |

- **528,853,952 words** · **~793M subword tokens** (×1.5) · **~4.0 GB** `data/final/final_so.jsonl`
- Deep-clean reject breakdown: boilerplate 23,948 · not_somali 6,906 · too_long 1,060 ·
  mostly_numbers 117 · html_remnant 23
- v0.1 → v0.2: 1,774,891 → 1,668,080 docs (−6.0%); 591M → 529M words (−10.5%)
- Audit highlights: URL remnants 18.8% → 0.07%; escaped `\n` 10.5% → 0.01%;
  boilerplate 4.3% → 1.06%; URLs masked to `⟨url⟩` sentinel in 16.17% of docs

## [0.1.0] — 2026-07-07

### Added

- Rust workspace with `common` and `corpus-tools` crates
- Minimal `Document` record type (`text` + optional `source`)
- Public dataset downloaders: HPLT, CC100, mC4, OPUS, MADLAD, MT560
- `merge_corpora` tool to combine raw JSONL sources
- `corpus-pipeline` crate: clean, LID (`lingua`), near-dedup (MinHash + LSH)
- `run_pipeline` stage runner and per-stage stats reports
- Documentation: architecture, pipeline, plan, roadmap, source catalog

### Corpus results (v0.1 baseline, no deep clean)

- 2.63M raw → **1,774,891** final documents · **591M words** · **~887M tokens** · ~4.5 GB
