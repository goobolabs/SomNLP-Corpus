# Source Catalog

Every data source for SomNLP-Corpus: what it contributes, how we access it, and
estimated scale. See [PLAN.md](../PLAN.md) for the two-track strategy.

**Status legend:** done · in progress · planned · skipped

---

## Track A — Public datasets

Pre-built Somali web corpora. Downloaded via `corpus-tools` binaries.

| Key | Source | Access | Est. raw scale | Tool | Status |
|-----|--------|--------|----------------|------|--------|
| `hplt` | [HPLT2.0 cleaned](https://huggingface.co/datasets/HPLT/HPLT2.0_cleaned) `som_Latn` | Hugging Face | ~505M tokens, ~966K docs | `download_hplt_so` | done |
| `cc100` | [CC-100 Somali](https://data.statmt.org/cc-100/so.txt.xz) | Direct HTTP | ~81M tokens, ~396K docs | `download_cc100_so` | done |
| `mc4` | [allenai/c4](https://huggingface.co/datasets/allenai/c4) `so` | Hugging Face | tens of millions | `download_mc4_so` | done |
| `madlad` | [MADLAD-400](https://huggingface.co/datasets/allenai/MADLAD-400) `so` | Hugging Face | tens of millions | `download_madlad_so` | done |
| `opus` | [opus_paracrawl](https://huggingface.co/datasets/Helsinki-NLP/opus_paracrawl) `en-so` | Hugging Face | parallel sentences | `download_opus_so` | done |
| `mt560` | [MT560 en–so pairs](https://huggingface.co/datasets/michsethowusu/english-somali_sentence-pairs_mt560) | Hugging Face | ~161K pairs | `download_mt560_so` | done |

### Cross-source overlap (approximate)

From baseline measurements on similar corpora:

- HPLT ∩ CC100: ~0.12% of documents overlap
- HPLT is the dominant source (~86% of raw tokens)
- mC4 and MADLAD share Common Crawl ancestry with HPLT — net new tokens after dedup will be lower than raw counts

### Skipped sources

| Source | Reason |
|--------|--------|
| OSCAR-2301 `so` | Only 6 documents / 51 words in the official split; language ID failed on Somali |

### Track A total (after dedup + quality filter)

Estimated **~250–350M final tokens** from public datasets alone.

---

## Track B — Collected sources

Targeted collection of Somali text not fully covered by public dumps.

### B1 — Web scraping (planned)

Somali text from news sites, blogs, government pages, universities, religious
articles, and online magazines.

| Parameter | Estimate |
|-----------|----------|
| Target sites | ~100 websites |
| Articles per site | ~500 |
| Words per article | ~800 |
| **Total** | **~40M words (~50M+ tokens)** |

Likely the largest Track B contributor. Requires per-site extraction rules,
robots.txt compliance, and rate limiting.

**Status:** planned

### B2 — Wikipedia & Wikimedia (planned)

| Source | Est. contribution |
|--------|-------------------|
| Somali Wikipedia | bulk of estimate |
| Wiktionary, Wikiquote, Wikinews | smaller additions |
| **Total** | **1–3M tokens** |

Well-structured, educational content. Relatively small but high quality.

**Status:** planned

### B3 — Books & educational materials (planned)

Public-domain books, author-approved texts, school materials, research publications.

| **Total** | **10–20M tokens** |

High grammatical quality. Per-source access review required.

**Status:** planned

### B4 — Social media (planned)

Public posts, comments, forums, community discussions.

| **Total** | **5–15M tokens** |

Reflects real-world language but needs heavy cleaning (spelling errors, emojis,
duplicates, informal writing).

**Status:** planned

### B5 — Subtitles (planned)

Movies, educational videos, public subtitle collections.

| **Total** | **5–10M tokens** |

Conversational Somali; everyday vocabulary and dialogue patterns.

**Status:** planned

### B6 — OCR digitization (planned)

Scanned old books, newspapers, dictionaries, printed archives.

| **Total** | **5–15M tokens** |

Requires OCR extraction plus manual or automated quality verification.

**Status:** planned

### B7 — Community contributions (planned)

Stories, poems, essays, articles, documentation submitted by the Somali community.

| **Total** | **several million tokens** (ongoing) |

Preserves cultural and dialect diversity. Grows over time.

**Status:** planned

---

## Combined corpus outlook

| Track | Est. final tokens |
|-------|-------------------|
| A — public datasets (after dedup/filter) | ~250–350M |
| B — collected sources | ~80–120M+ |
| **Combined potential** | **~330–470M+ tokens** |

Exact numbers depend on deduplication, quality filtering, and how much of Track B
is successfully collected.

---

## Output paths

| Stage | Path |
|-------|------|
| Per-source raw | `data/raw/<key>/<key>_so.jsonl` |
| Merged raw | `data/merged/merged_so.jsonl` |
| Cleaned | `data/cleaned/` (planned) |
| Deduplicated | `data/deduplicated/` (planned) |
| Filtered | `data/filtered/` (planned) |
| Final | `data/final/` (planned) |
