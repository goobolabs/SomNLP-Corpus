# Phase 3 Processing Plan

The authoritative specification for the SomNLP-Corpus processing pipeline: merge,
clean, language identification, and near-deduplication. Decisions here are locked
after senior review. Implementation must follow this document; where it disagrees
with the Rust types in `crates/common/src/types.rs`, the types win and this doc is
updated to match.

Companion docs: [DATA_PIPELINE.md](DATA_PIPELINE.md) (stage overview),
[METADATA_SCHEMA.md](METADATA_SCHEMA.md) (record shapes),
[ID_STRATEGY.md](ID_STRATEGY.md) (hashing and IDs),
[MERGE_SEMANTICS.md](MERGE_SEMANTICS.md) (merge behavior),
[QUALITY_METADATA.md](QUALITY_METADATA.md) (dedup and quality fields).

---

## Pipeline overview

```text
raw/
 → merge + provenance + exact dedup        (data/merged/)
 → clean                                    (data/cleaned/)
 → language identification                  (data/lid/)
 → near dedup                               (data/final/)
 → final output + stats + reject sidecars
```

Quality filtering (char-n-gram coverage against a clean seed) is **deferred** to a
later phase that depends on Wikipedia-so (Track B). See [§5](#5-quality-filtering-deferred).

Design priorities, in order: **simplicity, correctness, maintainability** over
sophistication. Every stage streams except near-dedup. Every stage writes a stats
report and routes rejected records to a sidecar instead of deleting them.

---

## Source classes

Two classes, declared per source in the registry ([SOURCES.md](SOURCES.md)) and read
from configuration. Class controls length floors, LID policy, and near-dedup
participation.

| Class | Sources | Unit | Length floor | LID policy | Near-dedup |
|-------|---------|------|--------------|------------|------------|
| `document` | HPLT, CC100, mC4, MADLAD | full documents | ~50 words | full gate | exact + MinHash |
| `sentence` | OPUS, MT560 | aligned sentences | ~3–5 words | tag-only / low threshold | exact only |

Rationale for splitting near-dedup by class is in [§Near-dedup](#near-deduplication).

---

## 1. Exact dedup during merge

**Decision: keep exact dedup inside the merge pass.** Streaming hash-set dedup is
O(1) extra memory per document (a few hundred MB of hashes at our scale) and removes
the bulk of redundancy before any expensive stage. Reference corpora showed ~14% of
documents removed here, including ~17% byte-identical duplicates inside HPLT alone.

Merge does:

- Stream each source in explicit priority order.
- Normalize text for hashing only (see [ID_STRATEGY.md](ID_STRATEGY.md): NFC, trim,
  collapse whitespace, lowercase). The stored `text` is **not** modified at merge.
- SHA-256 the normalized form; **first-seen wins**.
- Write the registry `source` key into every merged record.
- Record per-source within-source and cross-source duplicate counts.

### Hash timing vs. canonical IDs (correctness)

The merge-time hash is **dedup-only and never persisted** as `content_hash`. The
clean stage changes text (mojibake repair, entity decoding, whitespace), so a hash
taken at merge would no longer match the stored text.

- **Merge hash** — internal, transient, used only to drop raw exact duplicates.
- **Canonical `content_hash` and `DocId`** — computed once, on **cleaned** text, at
  the end of the clean stage.

### Post-clean exact-dedup recheck

Cleaning can make two raw-distinct documents byte-identical (e.g. a mojibake copy
and its clean twin; mojibake affects a large fraction of input). Add a second exact
hash-set pass at the **end of clean**, over canonical `content_hash`. It is one
membership check per document and guarantees the invariant: *no exact duplicates
survive the clean stage.*

### Source priority order (open, pick before implementation)

First-seen-wins requires a deterministic order. Working proposal, curated/parallel
first and largest web crawl last:

```text
mt560 → opus → cc100 → mc4 → madlad → hplt
```

Revisit once real merge overlap reports exist. The order must be fixed in config for
reproducibility.

---

## 2. Mojibake repair

**Decision: targeted Rust fixer using `encoding_rs` round-trips.** No full ftfy port.
This addresses the dominant artifact family (UTF-8 mis-decoded as a single-byte code
page), which affected ~52% of reference documents — the single highest-impact
cleaning step.

Required safety guards:

1. **Use Windows-1252, not Latin-1.** The common artifacts (`â€™`, `â€œ`, smart
   quotes, dashes) originate in the CP1252 0x80–0x9F block, which Latin-1 cannot
   reproduce.
2. **Gate on indicator patterns.** Only attempt a round-trip when indicators are
   present (`Ã`, `â€`, `Â `, `â€™`, …). Never round-trip unconditionally — a clean
   document containing `é` or `ñ` would be corrupted.
3. **Accept only on improvement.** Keep the repaired text only if it reduces the
   indicator count and introduces zero U+FFFD. Otherwise keep the original.
4. **Iterate to a fixed point, max 3 passes.** Double-encoding can occur more than
   once.

Build a ~50-pair golden test set sampled from **actual HPLT/CC100 lines** containing
`Ã`/`â€`, verified by eye. Do not rely on synthetic examples.

---

## 3. Language identification

**Decision: benchmark before choosing a library.** The clearest lesson from prior
work: the "obvious" choice can be the worst (fastText lid.176 measured ~7.5% recall
on Somali). Do not commit to a library up front.

Candidates: `lingua-rs`, `whatlang`, GlotLID (fastText bindings).
Measure **recall, precision, speed**.

Eval-set requirements:

1. **Independent positives and hard negatives — use FLORES-200.** It provides
   `som_Latn` plus the exact confusable languages (Swahili, Oromo, Afar, Amharic,
   plus English/Italian/Arabic). Do **not** draw eval positives from the corpora we
   are about to filter — that is circular.
2. **Bucket by length** (5, 10, 25, 50+ words). All LID degrades on short text, and
   sentence-class sources live entirely in the short buckets. A document-winner may
   lose on 5-word inputs; `lingua-rs`'s headline claim is short-text accuracy, so the
   comparison is genuinely open.

Policy by source class:

- **Document sources** — full LID gate; drop below the chosen confidence threshold.
- **Sentence sources** — tag-only (record `lang_score`, do not reject) or a much
  lower threshold. A document-tuned gate over 4-word aligned sentences false-rejects
  heavily while catching almost nothing.

The confidence threshold is an **output of the benchmark**, not a copied default.

---

## 4. Source classes and length floors

**Decision: two classes, not a single threshold.** A uniform 50-word floor would
erase the sentence sources; a uniform low floor would admit short web fragments. The
class lives in the registry and is read from config.

- Document class floor: ~50 words (matches HPLT's own floor; reference dropped ~8.6%,
  all too-short).
- Sentence class floor: ~3–5 words.

Class also gates near-dedup (next section) and LID policy (above).

---

## 5. Quality filtering (deferred)

**Decision: defer seed-based char-n-gram quality filtering.** It was the last filter
in prior work and depends on a clean Somali seed (Wikipedia-so), which arrives with
Track B. Everything before it stands alone, so Phase 3 ships:

```text
merge → clean → LID → near dedup → final
```

Two notes:

- **Zero-cost heuristic flags now (optional).** `QualityFlag::HighSymbolRatio`,
  `MostlyNumbers`, and `TooLong` need no seed corpus — they are one-pass arithmetic.
  Emitting them as flags with `disposition = review` (not reject) during clean costs
  almost nothing and provides data to calibrate thresholds later.
- **Document the retained tail.** v0.1 keeps the bottom-quality fraction that the
  deferred filter would remove (song-title spam, listing pages). State this in the
  release notes.

---

## Clean stage specification

Operations apply per record in this order. Order matters: entity decoding and
mojibake repair precede NFC; hashing is last.

```text
1. HTML entity decode      (once, all entities)
2. Mojibake repair         (CP1252 round-trip, guarded)
3. Unicode NFC
4. Strip control + invisible characters
5. Repeated-character collapse
6. Whitespace normalization
7. Length / empty / U+FFFD checks  → reject sidecar
8. Compute content_hash + DocId    → post-clean exact-dedup recheck
```

### HTML entities

Decode **all** entities exactly **once** using a real decoder (e.g. the `html-escape`
crate). Hardcoding a handful of named entities misses numeric forms (`&#8217;`,
`&#x27;`), which are at least as common. Decoding once avoids over-decoding chains
like `&amp;amp;`.

Literal tags (`<p>`, `<br>`) are **flagged** (`QualityFlag::HtmlRemnant`), **not
stripped**, in v1. Tag stripping is where conservative cleaning turns destructive;
defer it.

### Unicode NFC

Apply NFC after mojibake repair, before hashing. NFC does **not** remove invisible
characters (next item) — that is a separate step.

### Control and invisible characters

Remove control characters except `\n` and `\t`. Additionally strip web junk that NFC
leaves behind and that poisons shingling and exact-hash matching:

- Zero-width: U+200B, U+200C, U+200D
- BOM / zero-width no-break space: U+FEFF
- Soft hyphen: U+00AD
- Bidi controls: U+202A–U+202E

### Repeated-character collapse

Collapse pathological repetitions while preserving natural Somali spelling (Somali
long vowels are doublets, never legitimate triples, so a floor of 3 is safe).

- 4+ repeated **letters** → 3 (`waaaaaaa` → `waaa`, keeps `waaa`)
- 4+ repeated **punctuation** → 3 (`!!!!!!!` → `!!!`)
- **Never touch digits.** The naive `(.)\1{3,}` rule corrupts numbers
  (`10000000` → `1000`); digits must be excluded.

### Whitespace normalization

Per line: collapse internal whitespace runs to a single space and trim. Drop empty
lines or cap consecutive newlines at 2. **Preserve paragraph breaks** — they are real
signal for pretraining. Do not flatten the whole document to single spaces.

### Empty and corruption checks

- Empty after cleaning → reject (sidecar, reason recorded).
- U+FFFD: run **after** mojibake repair. Any occurrence → flag; ratio above ~0.5% →
  reject as heavily corrupted.

### Length floor

Apply the per-class floor from the registry (document ~50 words, sentence ~3–5).
Below floor → reject sidecar.

---

## Near-deduplication

**Decision: MinHash + LSH, applied to the document class only.**

Configuration (literature-standard, validated at ~1M documents):

| Parameter | Value |
|-----------|-------|
| Shingles | word 3-grams |
| MinHash permutations `k` | 64 |
| LSH bands × rows | 16 × 4 |
| Similarity threshold `τ` | 0.80 |
| Keep rule | longest document |
| RNG seed | fixed constant in config |

Requirements:

1. **Exact-Jaccard verification is mandatory.** LSH at (b=16, r=4) has its S-curve
   midpoint near (1/16)^(1/4) ≈ 0.5, so it produces ~0.5-similarity **candidates**,
   not a 0.8 filter. Each candidate pair must be verified against the real τ=0.80
   threshold before removal. Skipping this would delete ~0.5-similar documents — far
   too aggressive.
2. **Sentence class is excluded.** Word-3-gram shingling of a 4-word sentence yields
   ~2 shingles; MinHash over 1–3 shingles is noise and would create false-positive
   clusters that delete legitimate distinct sentences. For parallel/sentence data,
   exact dedup is the correct and sufficient duplicate semantic.
3. **Seed the permutation RNG** with a fixed constant in config; an unseeded run is
   not reproducible.

Memory: ~1M document-class records × ~400 unique shingle-ints × 8 bytes ≈ 3 GB using
integer arrays (not string sets). Acceptable on a workstation at our scale; revisit
above ~10M document-class records.

---

## Reproducibility and scalability

- **One versioned config** holds every knob (source order, length floors, LID
  threshold, near-dedup parameters, RNG seeds). Changed knobs are logged.
- **Fixed RNG seeds** for MinHash permutations.
- **Per-stage stats reports** and **reject sidecars** (already the schema design).
- **Stamp `schema_version` and pipeline version** into outputs.
- Everything streams except near-dedup, whose memory profile is bounded at our scale.

---

## Open items to settle before/at implementation

1. Source priority order for first-seen dedup (proposal above; confirm with overlap
   reports).
2. LID library and confidence threshold (outputs of the benchmark).
3. Exact sentence-class length floor within the 3–5 word range.
4. U+FFFD reject ratio (proposed ~0.5%).
