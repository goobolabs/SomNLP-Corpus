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

Build the stages between merged raw text and a training-ready corpus. Full
specification: [docs/CLEANING_PLAN.md](docs/CLEANING_PLAN.md).

```text
merge + exact dedup → clean → language identification → near dedup → final
```

- [ ] Merge: write `source` per record, streaming exact dedup (first-seen wins)
- [ ] Clean crate — HTML entities, mojibake repair (CP1252), NFC, control/invisible
      stripping, repeated-char collapse, whitespace, per-class length floors
- [ ] Canonical `content_hash` + `DocId` on cleaned text; post-clean exact recheck
- [ ] Language identification — benchmark `lingua-rs` / `whatlang` / GlotLID on a
      FLORES-200 eval set, then gate by source class
- [ ] Near-dedup — MinHash word-3-gram, k=64, LSH 16×4, τ=0.80, keep-longest,
      document class only, with exact-Jaccard verification
- [ ] Wire `CorpusRecord` metadata: provenance, lang scores, dedup info, quality flags
- [ ] Stage runner chaining merge → clean → LID → near-dedup, config-driven
- [ ] Per-stage stats reports + reject sidecars

Quality filtering (char-n-gram coverage) is deferred until Wikipedia-so lands in
Phase 4 to serve as the clean seed.

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
