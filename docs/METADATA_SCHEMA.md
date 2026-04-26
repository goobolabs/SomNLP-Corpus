# Metadata Schema

How corpus records carry metadata from download through final release.

The Rust types in `crates/common/src/types.rs` are the canonical contract.
This document describes field semantics and evolution rules.

**Current schema version:** `1`

---

## Design goals

1. **Per-source licensing** — no single corpus-wide license assumption.
2. **Provenance preservation** — every record traceable to an origin.
3. **Failed records kept** — quality and filter outcomes recorded, not silently dropped.
4. **Gradual growth** — raw records today; full metadata added as pipeline stages land.
5. **Scale** — flat JSONL records, no nested blobs required for 300M+ tokens.

---

## Record shapes by pipeline stage

```text
download/merge     →  RawRecord        { text, source? }
clean/filter/dedup →  CorpusRecord     { id, text, provenance, license, … }
```

See [MERGE_SEMANTICS.md](MERGE_SEMANTICS.md) for the merge step.
See [QUALITY_METADATA.md](QUALITY_METADATA.md) for dedup and quality fields.

---

## Raw records (implemented today)

Used in `data/raw/` and `data/merged/`.

| Field | Required | Type | Description |
|-------|:--------:|------|-------------|
| `text` | yes | string | Document text, UTF-8 |
| `source` | no | string | Registry key (`hplt`, `cc100`, …) when known |

```json
{"text": "Soomaaliya waa dal ku yaal Geeska Afrika."}
```

```json
{"text": "…", "source": "mt560"}
```

**Note:** `merge_corpora` currently writes `text` only. Source provenance is tracked
in merge statistics but not yet written into each JSONL line. The target behavior
is documented in [MERGE_SEMANTICS.md](MERGE_SEMANTICS.md).

Rust type: `RawRecord` (alias `Document`).

---

## Corpus records (target from cleaning onward)

| Field | Required | Type | Description |
|-------|:--------:|------|-------------|
| `id` | yes | string | Stable document ID ([ID_STRATEGY.md](ID_STRATEGY.md)) |
| `text` | yes | string | Cleaned UTF-8 plain text |
| `provenance` | yes | object | Origin metadata (below) |
| `license` | yes | string | Per-source license ([SOURCES.md](SOURCES.md)) |
| `content_hash` | yes | string | SHA-256 hex of normalized text |
| `dedup` | yes | object | Duplicate metadata |
| `quality` | yes | object | Quality gate outcomes |
| `schema_version` | yes | u16 | Currently `1` |
| `meta` | no | object | Source-specific extensions |

Rust type: `CorpusRecord`.

---

## Provenance (required on CorpusRecord)

| Field | Required | Type | Description |
|-------|:--------:|------|-------------|
| `source` | yes | string | Registry key from [SOURCES.md](SOURCES.md) |
| `collected_at` | yes | RFC 3339 UTC | When this record entered the pipeline |
| `lang` | yes | string | BCP-47 tag (expected `"so"` for kept records) |
| `source_url` | no | string | Canonical URL of original document |
| `title` | no | string | Headline or article title |
| `author` | no | string | Author or organization |
| `published_at` | no | RFC 3339 UTC | Original publication date |
| `tags` | no | string[] | Domain/topic labels |
| `subsource` | no | string | Finer origin (e.g. `so.wikipedia.org`, `bbc.com/somali`) |

### Required metadata (summary)

Across the full pipeline, every **CorpusRecord** must have:

- **source** — which catalog entry produced the text
- **collected_at** — when we captured it
- **lang** — assigned language tag

`text` and `id` are required at the record level.

### Optional metadata (summary)

- `source_url`, `title`, `author`, `published_at`, `tags`, `subsource`
- Arbitrary keys in `meta` (namespaced by source key to avoid collisions)

---

## Licensing model

SomNLP-Corpus does **not** ship under one global license. Each source has its own
terms. The `license` field on `CorpusRecord` carries the SPDX-style identifier for
**that record's source**.

| Approach | Description |
|----------|-------------|
| Per-record `license` | Copied from source registry at processing time |
| Per-source registry | Canonical license listed in [SOURCES.md](SOURCES.md) |
| Redistribution | Downstream users filter or attribute per source |

Known license variants in `License` enum:

- `CC0-1.0`, `CC-BY-4.0`, `CC-BY-SA-4.0`, `MIT`, `Apache-2.0`, `public-domain`
- `Other("…")` for source-specific terms

When a new source is added, document its license in [SOURCES.md](SOURCES.md) before
ingestion. Do not assume all sources share the same redistribution rights.

---

## Source-level manifest (planned)

Each download run will eventually write a sidecar manifest:

```json
{
  "source": "hplt",
  "tool": "download_hplt_so",
  "tool_version": "0.1.0",
  "downloaded_at": "2026-06-10T12:00:00Z",
  "upstream": "HPLT/HPLT2.0_cleaned/som_Latn",
  "license": "CC0-1.0",
  "record_count": 966507,
  "output": "data/raw/hplt/hplt_so.jsonl",
  "sha256": "…"
}
```

Manifests support reproducibility and per-source license tracking without embedding
license strings in every raw line.

---

## Schema evolution rules

| Change | Action |
|--------|--------|
| Add optional field | Minor; same `schema_version` |
| Add required field | Bump `schema_version`; migration note |
| Change ID or hash formula | Bump `schema_version`; invalidate processed data |
| Rename field | Bump `schema_version` |

The Rust types in `crates/common/src/types.rs` are authoritative. When docs and
types disagree, the types win.
