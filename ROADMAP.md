# Roadmap

Phases and milestones for SomNLP-Corpus. See [PLAN.md](PLAN.md) for strategy and
[docs/SOURCES.md](docs/SOURCES.md) for the source catalog.

```text
M1 Foundation ─▶ M2 Public datasets ─▶ M3 Pipeline ─▶ M4 Collection ─▶ M5 Release
   (done)            (done)              (done)          (next)          (planned)
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

## Phase 3 — Processing pipeline (done)

Build the stages between merged raw text and a training-ready corpus. Full
specification: [docs/CLEANING_PLAN.md](docs/CLEANING_PLAN.md).

```text
merge + exact dedup → clean → language identification → deep clean → near dedup → final
```

- [x] Merge: write `source` per record, streaming exact dedup (first-seen wins)
- [x] `corpus-pipeline` crate — HTML entities, mojibake repair (CP1252), NFC,
      control/invisible stripping, repeated-char collapse, whitespace, per-class
      length floors
- [x] Canonical `content_hash` + `DocId` on cleaned text; post-clean exact recheck
- [x] Language identification — benchmark `lingua` / `whatlang` on labeled eval
      set; gate document-class, tag-only sentence-class
- [x] Near-dedup — MinHash word-3-gram, k=64, LSH 16×4, τ=0.80, keep-longest,
      document class only, with exact-Jaccard verification
- [x] `CorpusRecord` metadata: provenance, lang scores, dedup info, quality flags
- [x] Stage runner (`run_pipeline`) chaining merge → clean → LID → deep_clean → near-dedup
- [x] Per-stage stats reports + reject sidecars

Quality filtering (char-n-gram coverage) is deferred until Wikipedia-so lands in
Phase 4 to serve as the clean seed.

**Exit:** reproducible pipeline from `data/merged/` to `data/final/` with stats
reports. See [docs/DATA_PIPELINE.md](docs/DATA_PIPELINE.md) for commands.

---

## Phase 3.5 — v0.2 deep clean (done)

Second-pass cleaning on LID-verified records before near-dedup. Audit and priorities:
[docs/CLEANING_STRATEGY.md](docs/CLEANING_STRATEGY.md).

```text
merge → clean → LID → deep_clean → near dedup → final
                      deep_clean/              final/
```

- [x] `deep_clean` binary — source-aware normalize, HTML/contact, boilerplate, segment LID, intra-doc dedup
- [x] Near-dedup reads `data/deep_clean/deep_clean_so.jsonl`; release output stays `data/final/final_so.jsonl`
- [x] Stage reports renumbered: `04_deep_clean_stats.json`, `05_near_dedup_stats.json`
- [x] `run_pipeline` chains all five post-merge stages
- [x] Full-corpus v0.2 re-run and audit re-measurement (`reports/06_cleaning_audit.md`)

**Exit (met):** 2.63M raw → **1,668,080** final docs · **529M words** · ~4.0 GB
`data/final/final_so.jsonl`. v0.1 baseline was 1.77M docs · 591M words.

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

## Phase 6 — Release v0.2-clean

- [x] Corpus statistics report (docs, tokens, domains, length distributions)
- [x] Cleaning audit (`reports/06_cleaning_audit.md`)
- [ ] Dataset card and changelog
- [ ] Hugging Face dataset upload
- [ ] Tagged release with checksums

**Exit:** a downloadable, documented, citable `v0.2-clean` corpus on Hugging Face.

---

## Milestone tracker

| Milestone | Theme | Status |
|-----------|-------|--------|
| M1 | Foundation | done |
| M2 | Public dataset download + merge | done |
| M3 | Cleaning, dedup, langid pipeline | done |
| M3.5 | v0.2 deep clean + full pipeline run | done |
| M4 | Web & Wikipedia collection | planned |
| M5 | Extended sources | planned |
| M6 | Release v0.2-clean (Hugging Face) | in progress |
