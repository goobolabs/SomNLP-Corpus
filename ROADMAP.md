# Roadmap

Phases and milestones for SomNLP-Corpus. See [PLAN.md](PLAN.md) for strategy and
[docs/SOURCES.md](docs/SOURCES.md) for the source catalog.

```text
M1 Foundation ─▶ M2 Public datasets ─▶ M3 Pipeline ─▶ M4 Collection ─▶ M5 Release
   (done)            (done)              (next)          (planned)       (planned)
```

---

## Phase 1 — Foundation (done)

- [x] Repository structure and tooling config
- [x] Cargo workspace with `common` crate
- [x] Minimal `Document` record type (`text` + optional `source`)

## Phase 2 — Public dataset tooling (done)

- [x] `corpus-tools` crate with shared download utilities
- [x] Downloaders: HPLT, CC100, mC4, OPUS, MADLAD, MT560
- [x] `merge_corpora` to combine raw JSONL sources
- [x] Architecture and pipeline documentation

**Exit:** `cargo build --release` succeeds; downloaders and merge tool are runnable.

---

## Phase 3 — Processing pipeline (next)

Build the stages between merged raw text and a training-ready corpus.

- [ ] Cleaning crate — encoding fix, HTML strip, whitespace, Somali-aware rules
- [ ] Deduplication — exact hash + near-duplicate (MinHash/LSH)
- [ ] Language identification — filter non-Somali text
- [ ] Quality gates — length, symbol ratio, repeated n-grams
- [ ] Expand `Document` schema with lang scores, dedup info, quality flags
- [ ] Unified CLI or stage runner to chain: merge → clean → dedup → filter

**Exit:** reproducible pipeline from `data/merged/` to `data/final/` with a stats report.

---

## Phase 4 — Web & Wikipedia collection

Add targeted Somali text beyond public dumps.

- [ ] Wikipedia & Wikimedia connector (Somali Wikipedia, Wiktionary, Wikiquote)
- [ ] Web scraper for Somali news, government, university, and blog sites
- [ ] Per-site extraction rules and robots.txt compliance
- [ ] Rate limiting and provenance metadata (URL, title, date)

**Exit:** at least 3 collected sources ingested end-to-end alongside public datasets.

---

## Phase 5 — Extended sources

- [ ] Books & educational materials ingestion
- [ ] Subtitle collections (movies, educational video)
- [ ] Social media & forum text (with heavy cleaning)
- [ ] OCR pipeline for scanned books and newspapers
- [ ] Community contribution intake

**Exit:** multi-domain corpus with documented per-source statistics.

---

## Phase 6 — Release v0.1.0

- [ ] Corpus statistics report (docs, tokens, domains, length distributions)
- [ ] Dataset card and changelog
- [ ] Hugging Face dataset upload
- [ ] Tagged release with checksums

**Exit:** a downloadable, documented, citable `v0.1.0` corpus.

---

## Milestone tracker

| Milestone | Theme | Status |
|-----------|-------|--------|
| M1 | Foundation | done |
| M2 | Public dataset download + merge | done |
| M3 | Cleaning, dedup, langid pipeline | planned |
| M4 | Web & Wikipedia collection | planned |
| M5 | Extended sources | planned |
| M6 | Release v0.1.0 | planned |
