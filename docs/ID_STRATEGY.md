# ID and Hash Strategy

Stable, deterministic identifiers for documents, content, and source runs.

Designed for 300M+ records, cross-source dedup, and reproducible releases.

---

## Goals

| Requirement | Approach |
|-------------|----------|
| Deterministic | Same text + source → same ID across re-runs |
| Reproducible | Hash inputs documented; no random UUIDs for content IDs |
| Stable across releases | ID formula versioned via `schema_version` |
| Dedup-friendly | Content hash over normalized text |
| Source-aware | IDs include source key to avoid cross-source collisions |

---

## Content hash

Used for exact dedup and as input to document IDs.

**Input:** normalized text (not the raw merge line)

**Normalization for hashing only** (does not change stored `text` until clean stage defines otherwise):

1. Unicode NFC
2. Trim leading/trailing whitespace
3. Collapse internal whitespace to single spaces
4. Lowercase (for hash stability; optional for Somali — document choice at clean stage)

**Algorithm:** SHA-256

**Output:** lowercase hex string (64 characters)

```text
content_hash = sha256(normalize(text))
```

Rust type: `ContentHash`.

---

## Document ID

**Format:**

```text
{source_key}:{content_hash_prefix}
```

- `source_key` — registry key from [SOURCES.md](SOURCES.md) (`hplt`, `cc100`, `wikipedia`, …)
- `content_hash_prefix` — first 16 hex chars of `content_hash`

**Example:**

```text
hplt:a3f8c2b91e004d81
```

### Why source + hash prefix

| Concern | Resolution |
|---------|------------|
| Same text in two sources | Different IDs (provenance preserved) |
| Exact same text re-ingested | Same ID (deterministic) |
| Hash collision (truncated) | Full 256-bit hash stored in `content_hash`; prefix is convenience |
| Cross-source dedup | Compare full `content_hash`, not `id` |

Rust type: `DocId`.

---

## Source run identifiers (planned)

For manifests and reproducibility, each download/collection run gets:

```text
run_id = {source_key}/{YYYYMMDD}/{short_hash}
```

Example: `hplt/20260610/9f3a1c0b`

The short hash covers: tool version + upstream URL/version + output file checksum.

---

## Near-duplicate fingerprints (planned)

Exact hash is insufficient for near-dedup. Planned additions:

| Method | Use |
|--------|-----|
| MinHash + LSH | Near-duplicate clusters at Jaccard threshold τ ≈ 0.80 |
| SimHash (optional) | Fast near-duplicate pre-filter at very large scale |

Near-dup metadata references the canonical record via `near_duplicate_of: DocId`.

---

## Parent-child relationships (future)

Sentence-level artifacts will use derived IDs:

```text
{doc_id}#{sentence_index}
```

Example: `hplt:a3f8c2b91e004d81#0`

Sentences are reproducible from parent `CorpusRecord.text`; they are never authoritative.

---

## ID stability across pipeline stages

| Stage | ID assigned? | Hash assigned? |
|-------|:------------:|:--------------:|
| Raw / merge | no | no |
| Clean | yes (`DocId`) | yes (`content_hash`) |
| Dedup | unchanged | unchanged |
| Lang filter | unchanged | unchanged |
| Final export | unchanged | unchanged |

Once assigned at clean, `id` and `content_hash` are **immutable** for that record.

---

## Schema version coupling

If the ID formula or hash normalization changes:

1. Increment `schema_version` in `crates/common/src/types.rs`
2. Document in CHANGELOG
3. Reprocess from `data/merged/` or `data/raw/`

Do not mix records from different ID formula versions in one release.
