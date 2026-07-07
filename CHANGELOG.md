# Changelog

All notable changes to SomNLP-Corpus are documented here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **Somali BPE tokenizer pipeline** (`tokenizer/`) — prepare, train, and benchmark scripts
  using Hugging Face `tokenizers` on the final release corpus
- Trained **32k vocabulary** model: `tokenizer/somali-bpe-tokenizer.json`
- Full-corpus benchmark: mean **1.53** tokens/word (native) vs **2.69** (BERT-base) vs
  **1.94** (XLM-RoBERTa) on 1,668,080 documents
- Technical note: [tokenizer/PAPER.md](tokenizer/PAPER.md)

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
