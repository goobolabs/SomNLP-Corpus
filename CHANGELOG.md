# Changelog

All notable changes to SomNLP-Corpus are documented here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **Thirteen-source measured run (2026-09-02)** — full Track A pipeline on all downloaders:
  17.0M raw → 10.79M merged → 7,352,961 final documents · 666M words ·
  **911,824,557 v2 subword tokens**, measured over every document rather than extrapolated
  from the holdout ratio
- **Glot500** (`glot`) and **Somali Web Corpus** (`somali-web-corpus`) downloaders, registry
  entries, and pipeline registration. Glot500 contributes 1,433,476 final documents (9.8% of
  tokens); Somali Web Corpus contributes 121,777 (0.7%)
- **`tokenizer/tokenize_corpus.py`** — full-corpus tokenization pass. Shards the corpus by
  byte range across a process pool, keeps counters only, and derives percentiles from a
  mergeable histogram, so peak memory is independent of corpus size
- Incremental measurement log: [reports/runs/MEASUREMENT.md](reports/runs/MEASUREMENT.md)
- **`merge_source_order`** in `configs/pipeline.toml` extended to all thirteen sources
- **Somali BPE tokenizer pipeline** (`tokenizer/`) — prepare, train, benchmark, and export
  scripts using Hugging Face `tokenizers` on the final release corpus
- **v2 tokenizer** (`tokenizer/somali-bpe-v2.json`, 48k ByteLevel BPE) trained on the full
  eleven-source corpus, plus a `transformers`-loadable export at `tokenizer/v2/`
- Deterministic held-out evaluation split (59,029 documents, `blake2b-8(id) mod 1000 < 8`).
  Hashing the record id keeps the split stable as the corpus grows: every id held out of the
  thirteen-source corpus was also held out of the eleven-source corpus v2 trained on
- Per-source composition stats in `tokenizer_stats.json` — document share and word share
  diverge sharply (NLLB is 55.9% of documents but 9.3% of words; Glot500 19.5% against 10.1%)
- Property test suite for the tokenizer (round-trip losslessness, zero-`<unk>` invariant,
  vocabulary-truncation equivalence) plus `pyproject.toml` with ruff and pytest config
- Technical note: [tokenizer/PAPER.md](tokenizer/PAPER.md)

### Fixed

- **Tokenizer decode was destroying whitespace.** v1's `BPEDecoder` was configured to strip
  an `</w>` suffix its trainer never emitted, so `decode(encode(x))` concatenated all tokens:
  `"Soomaaliya waa dal"` → `"Soomaaliyawaadal"`. It fails on **all 59,029** held-out
  documents, and on every document of the full corpus. v2's ByteLevel decoder round-trips exactly (fidelity 1.000).
- **Unknown-token loss.** v1 mapped out-of-alphabet characters to `<unk>` (4 held-out
  documents affected). v2 seeds all 256 bytes, making `<unk>` unreachable.
- **Benchmark memory.** The reservoir sampler held every sampled document in memory at once
  (an estimated 10–14 GB at full corpus size); it now streams and retains only per-document
  ratios.
- **Silent training on a stale corpus.** `train.py` checked only that the training text
  existed. It now compares a corpus fingerprint recorded by `prepare_corpus.py` and refuses
  to proceed on a mismatch unless `--allow-stale` is passed.
- **`train.py` could silently overwrite the shipped v2 artifact.** `--output` defaults to
  `somali-bpe-v2.json`, so a smoke-test run at a small vocabulary replaced the release model.
  It now refuses to overwrite an existing `--output` unless `--force` is passed.
- **`benchmark.py --sweep-dir` silently scored nothing.** Relative paths resolve against the
  repository root, not `tokenizer/`, so `--sweep-dir sweep` globbed a directory that does not
  exist and skipped every sweep candidate without a warning. It now fails with a clear error.
- **`somali_raw_corpus.txt` was never one document per line.** 18% of documents contain
  internal newlines, so v1's published "per-document" ratios were largely per-paragraph.
  The evaluation split is now JSONL, making the unit unambiguous.

### Changed

- v1 benchmark figures superseded. Under the corrected document-level protocol on the
  thirteen-source corpus, v1 measures 1.3885 mean tokens/word and v2 measures
  **1.3468**, against 2.6336 (BERT-base) and 1.8158 (XLM-RoBERTa).
  The previously published 1.53 is not comparable — it scored lines, not documents.
- Corpus token total is now measured rather than extrapolated. The previous 826M came from
  multiplying the word count by the holdout ratio; encoding every document gives
  **911,824,557**.
- v2 was **not** retrained when the corpus grew to thirteen sources. The two additions
  tokenize better than the corpus mean under the existing merge table — `glot` 1.3370 and
  `somali-web-corpus` 1.2472 against a holdout mean of 1.3468 — so a retrain would move the
  headline by less than the vocabulary sweep's smallest step.
- Test fixture extended to 210 documents spanning all thirteen sources.
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
