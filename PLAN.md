# SomNLP-Corpus — Project Plan

A high-quality, reproducible **Somali** text corpus for NLP, LLM, and AI research.
**Rust-first** data pipeline.

## Vision

Somali (`so`) is a low-resource language. Clean, well-documented Somali text is scarce,
which holds back NLP and LLM development. **SomNLP-Corpus** builds a large, cleaned,
deduplicated Somali corpus as the foundation for downstream tasks: translation, NER,
QA, instruction tuning, sentiment, speech, and evaluation.

## Strategy

Corpus growth follows two tracks, in order:

```text
Track A — public datasets     (download + merge)     ← implemented
Track B — collected sources   (crawl + ingest)       ← planned
         ├── web scraping (news, gov, blogs, …)
         ├── Wikipedia & Wikimedia
         ├── books & educational materials
         ├── social media & forums
         ├── subtitles
         ├── OCR digitization
         └── community contributions
```

**Track A first.** Public web-crawl datasets (HPLT, CC100, mC4, MADLAD, etc.) provide
the bulk of raw Somali text quickly and reproducibly. **Track B second.** Targeted
collection adds fresher, higher-quality, domain-specific text that public dumps miss.

## Current status

| Stage | Status |
|-------|--------|
| Public dataset downloaders | Done (`corpus-tools`) |
| Merge raw sources | Done (`merge_corpora`) |
| Cleaning & normalization | Planned |
| Deduplication | Planned |
| Language filtering | Planned |
| Web & Wikipedia collection | Planned |
| Release packaging | Planned |

## Design principles

1. **Start with what exists** — download public datasets before building crawlers.
2. **Streaming & reproducible** — config-driven tools, fixed outputs, JSONL throughout.
3. **Grow the schema with the pipeline** — record types expand as stages are added.
4. **Quality over noise** — measure and document what is kept vs. dropped.
5. **Provenance** — every document should eventually carry source, URL, and date.

## Technology

| Concern | Approach |
|---------|----------|
| Dataset download & merge | **Rust** (`corpus-tools`) |
| Cleaning, dedup, langid | **Rust** (planned crates) |
| Web & Wikipedia collection | **Rust** (planned `collector` crate) |
| Analysis, HF upload, notebooks | **Python** (planned, thin layer) |

## Corpus size targets

### Track A — public datasets (available now)

| Source | Approx. raw scale |
|--------|-------------------|
| HPLT v2 (`som_Latn`) | ~505M tokens |
| CC100 | ~81M tokens |
| mC4 | tens of millions (overlaps with above) |
| MADLAD-400 | tens of millions |
| OPUS ParaCrawl | parallel sentences |
| MT560 | ~161K sentence pairs |

After cross-source dedup and quality filtering, Track A alone can reach **~250–350M
final tokens** — comparable to published Somali web corpora.

### Track B — collected sources (estimates)

| Source | Estimated contribution |
|--------|------------------------|
| Web scraping (~100 sites) | ~40M words (~50M+ tokens) |
| Wikipedia & Wikimedia | 1–3M tokens |
| Books & educational materials | 10–20M tokens |
| Social media & forums | 5–15M tokens |
| Subtitles | 5–10M tokens |
| OCR digitization | 5–15M tokens |
| Community contributions | several million (ongoing) |

Track B adds fresher, domain-rich text and long-term growth beyond public dumps.

## Documentation map

| Doc | Purpose |
|-----|---------|
| [PLAN.md](PLAN.md) | Vision, strategy, targets (this file) |
| [ROADMAP.md](ROADMAP.md) | Phases, milestones, timeline |
| [docs/SOURCES.md](docs/SOURCES.md) | Source catalog and token estimates |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Workspace and crate layout |
| [docs/DATA_PIPELINE.md](docs/DATA_PIPELINE.md) | Pipeline stages and usage |
| [README.md](README.md) | Quick start |
