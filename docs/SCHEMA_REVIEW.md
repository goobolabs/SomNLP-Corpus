# Schema Review

Senior review of schema and metadata decisions for SomNLP-Corpus.

Date: 2026-06-10. Schema version: `1`.

---

## Summary

The design supports 300M+ tokens, per-source licensing, reproducible IDs, and
gradual schema growth without rewriting download tooling. Complexity is intentionally
low at the raw stage and increases only when pipeline stages need it.

**Verdict:** Proceed with `RawRecord` today and `CorpusRecord` from cleaning onward.
Address the merge source-tag gap before the clean stage.

---

## Strengths

| Decision | Why it works |
|----------|--------------|
| Two record shapes (`RawRecord` → `CorpusRecord`) | Matches implemented pipeline; avoids over-modeling raw JSONL |
| Per-source `license` on `CorpusRecord` | Correct for an aggregation project with mixed upstream terms |
| `source:hash_prefix` document IDs | Deterministic, provenance-aware, dedup-friendly |
| Reject sidecars instead of silent drops | Critical for low-resource language stewardship |
| Flat JSONL at all stages | Proven at 300M+ scale; streamable; no DB required |
| `meta` map for extensions | New sources add fields without schema bumps |
| Registry keys in [SOURCES.md](SOURCES.md) | Single vocabulary for paths, IDs, and metadata |

---

## Issues found

### 1. Merge loses source provenance (high priority)

**Problem:** `merge_corpora` writes `{"text": "…"}` only. Source key is in stats, not output.

**Risk:** Clean stage cannot assign `provenance.source` without re-merging from raw files.

**Fix:** Write `source` on every merged line (documented in [MERGE_SEMANTICS.md](MERGE_SEMANTICS.md)).
Small code change; no schema version bump.

### 2. Raw `source` field is inconsistent

**Problem:** Only MT560 embeds `source` in raw JSONL today. Other downloaders emit text-only.

**Risk:** Inconsistent raw files until merge fix or downloader update.

**Fix:** Either merge adds source (preferred) or all downloaders add `source` at write time.

### 3. License strings not yet on raw records

**Problem:** License lives in registry docs only until `CorpusRecord` is built.

**Risk:** None for raw stage. Must be attached at clean stage from [SOURCES.md](SOURCES.md).

**Fix:** Clean stage copies `license` from registry by `source` key. No raw-stage change needed.

### 4. `License::Other` is a catch-all

**Problem:** Track B sources will have varied terms.

**Risk:** Low if every source is documented before ingestion.

**Fix:** Keep `Other(String)`; require SPDX or descriptive string in registry.

---

## Unnecessary complexity avoided

| Rejected | Reason |
|----------|--------|
| Full `CorpusRecord` at download time | Raw dumps lack URL, title, dates; premature |
| UUID document IDs | Non-reproducible across runs |
| Nested provenance history arrays | YAGNI; single `provenance` block suffices for v1 |
| Separate schema per source | Registry + `meta` map is enough |
| Graph DB or Parquet-only raw stage | JSONL streaming works; Parquet comes at export |

---

## Migration risks

| Risk | Mitigation |
|------|------------|
| ID formula change invalidates releases | Version `schema_version`; document in CHANGELOG |
| Hash normalization change re-clusters dedup | Bump version; re-run from merged |
| Adding required `CorpusRecord` fields | Prefer optional fields first; bump version if required |
| Merge format change (`source` field) | Backward-compatible addition to JSONL |

No migration needed for existing raw files when `source` is added to merge output.

---

## Missing metadata (acceptable for now)

| Field | When to add |
|-------|-------------|
| `n_chars`, `n_words` | Clean stage |
| `domain` (news, gov, …) | Clean or validate stage |
| `simhash` | Dedup stage if SimHash is used |
| Download manifests | Next downloader revision |
| Merge manifest | Next merge revision |
| Sentence-level records | Export stage |

---

## Scale check: 300M+ tokens

| Concern | Assessment |
|---------|------------|
| JSONL file size | Multi-file sharding at export if single file exceeds filesystem comfort |
| Record size | Text dominates; metadata adds ~500 bytes — negligible vs. mean doc length |
| Hash computation | Streaming SHA-256 per record; parallelizable in clean stage |
| ID collisions (16-char prefix) | Negligible at corpus scale; full hash retained |
| Cross-source dedup | Full `content_hash` comparison; source prefix does not block dedup |

Architecture supports target scale without structural changes.

---

## Future source readiness

| Source type | Schema readiness |
|-------------|------------------|
| Public datasets (Track A) | `source` key + registry license sufficient |
| Wikipedia | `provenance.source_url`, `title`, `subsource` ready |
| Web scraping | `source_url`, `title`, `published_at`, `tags` ready |
| Books / OCR | `author`, `published_at`, `meta` for OCR confidence |
| Community | `author`, `tags`, `meta` for submission ID |

No schema redesign expected for Track B.

---

## Recommended next steps

1. **Fix merge** to write `source` on each output line.
2. **Implement clean stage** that maps `RawRecord` → `CorpusRecord` with IDs and hashes.
3. **Add download/merge manifests** for reproducibility.
4. **Benchmark langid** on Somali before committing to a backend.
5. **Keep `RawRecord` alias `Document`** until all references use `RawRecord`.

---

## Document index

| Doc | Purpose |
|-----|---------|
| [METADATA_SCHEMA.md](METADATA_SCHEMA.md) | Field definitions and licensing |
| [SOURCES.md](SOURCES.md) | Source registry |
| [MERGE_SEMANTICS.md](MERGE_SEMANTICS.md) | Merge behavior and gaps |
| [QUALITY_METADATA.md](QUALITY_METADATA.md) | Dedup and quality outcomes |
| [ID_STRATEGY.md](ID_STRATEGY.md) | IDs and hashes |
| [SCHEMA_REVIEW.md](SCHEMA_REVIEW.md) | This review |

Rust types: `crates/common/src/types.rs`.
