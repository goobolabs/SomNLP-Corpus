# Source Registry

Formal catalog of all data sources for SomNLP-Corpus. Each source has a stable
registry key used in metadata, IDs, and file paths.

See also: [METADATA_SCHEMA.md](METADATA_SCHEMA.md), [PLAN.md](../PLAN.md).

**Status:** `done` · `planned` · `skipped`

---

## Registry — Track A (public datasets)

| Key | Name | Category | Track | Est. scale | License | Tool | Status | Notes |
|-----|------|----------|:-----:|------------|---------|------|--------|-------|
| `hplt` | HPLT2.0 cleaned `som_Latn` | web crawl | A | ~505M tokens, ~966K docs | CC0-1.0 | `download_hplt_so` | done | Primary backbone; dominates raw token count |
| `cc100` | CC-100 Somali | web crawl | A | ~81M tokens, ~396K docs | CC-BY-SA-4.0 | `download_cc100_so` | done | ~0.12% overlap with HPLT; net-additive |
| `mc4` | mC4 `so` | web crawl | A | tens of millions | ODC-BY | `download_mc4_so` | done | Overlaps CC ancestry with HPLT; dedup required |
| `madlad` | MADLAD-400 `so` | web crawl | A | tens of millions | ODC-BY | `download_madlad_so` | done | Clean split default; `--include-noisy` optional |
| `opus` | OPUS ParaCrawl `en-so` | parallel text | A | parallel sentences | CC0-1.0 | `download_opus_so` | done | Somali column from `translation.so` |
| `mt560` | MT560 en–so pairs | parallel / religious | A | ~161K pairs | CC-BY-4.0 | `download_mt560_so` | done | Writes `source: mt560` tag in raw JSONL |
| `quran` | QuranEnc Somali (Yacob Yusuf) | religious | A | ~6.2K verses + footnotes | see source | `download_quran_so` | done | Two outputs: verse translations and footnote explanations |

### Track A licensing note

There is **no single corpus license**. Redistribution of the combined corpus requires
honoring each upstream license. The `license` field on processed records is copied
from this registry (see [METADATA_SCHEMA.md](METADATA_SCHEMA.md)).

### Skipped — Track A

| Key | Name | Reason | Status |
|-----|------|--------|--------|
| `oscar` | OSCAR-2301 `so` | Official split: 6 docs / 51 words; LID failed on Somali | skipped |

### Track A outlook

~250–350M final tokens after cross-source dedup and quality filtering.

---

## Registry — Track B (collected sources)

| Key | Name | Category | Track | Est. scale | License | Tool | Status | Notes |
|-----|------|----------|:-----:|------------|---------|------|--------|-------|
| `web` | Somali web scraping | news / blogs / gov | B | ~40M words | per-site | collector (planned) | planned | ~100 sites; robots.txt required |
| `wikipedia` | Somali Wikipedia | encyclopedia | B | ~1–3M tokens | CC-BY-SA-4.0 | collector (planned) | planned | Pilot quality anchor |
| `wikimedia` | Wiktionary / Wikiquote / Wikinews | reference | B | small | CC-BY-SA-4.0 | collector (planned) | planned | Supplement to Wikipedia |
| `books` | Books & educational materials | literature / education | B | 10–20M tokens | per-work | collector (planned) | planned | Public domain or author-approved |
| `social` | Social media & forums | informal | B | 5–15M tokens | per-platform | collector (planned) | planned | Heavy cleaning required |
| `subtitles` | Video subtitles | conversational | B | 5–10M tokens | per-collection | collector (planned) | planned | Movies, educational video |
| `ocr` | OCR digitization | historical | B | 5–15M tokens | per-work | collector (planned) | planned | Scanned books, newspapers |
| `community` | Community contributions | mixed | B | ongoing | per-submission | intake (planned) | planned | Stories, poems, essays |

---

## Per-source detail

### `hplt`

- **Upstream:** [HPLT/HPLT2.0_cleaned](https://huggingface.co/datasets/HPLT/HPLT2.0_cleaned) config `som_Latn`
- **Access:** Hugging Face parquet shards
- **Output:** `data/raw/hplt/hplt_so.jsonl`
- **License:** CC0-1.0 (verify on upstream dataset card before release)

### `cc100`

- **Upstream:** [CC-100 Somali](https://data.statmt.org/cc-100/so.txt.xz)
- **Access:** Direct HTTP (xz compressed)
- **Output:** `data/raw/cc100/cc100_so.jsonl`
- **License:** CC-BY-SA-4.0

### `mc4`

- **Upstream:** [allenai/c4](https://huggingface.co/datasets/allenai/c4) config `so`
- **Access:** Hugging Face gzip-json shards (enumerated)
- **Output:** `data/raw/mc4/mc4_so.jsonl`
- **License:** ODC-BY (verify on upstream)

### `madlad`

- **Upstream:** [allenai/MADLAD-400](https://huggingface.co/datasets/allenai/MADLAD-400) language `so`
- **Access:** Hugging Face jsonl.gz shards
- **Output:** `data/raw/madlad/madlad_so.jsonl`
- **License:** ODC-BY (verify on upstream)

### `opus`

- **Upstream:** [Helsinki-NLP/opus_paracrawl](https://huggingface.co/datasets/Helsinki-NLP/opus_paracrawl) config `en-so`
- **Access:** Hugging Face parquet; field `translation.so`
- **Output:** `data/raw/opus/opus_so.jsonl`
- **License:** CC0-1.0 (verify on upstream)

### `mt560`

- **Upstream:** [english-somali_sentence-pairs_mt560](https://huggingface.co/datasets/michsethowusu/english-somali_sentence-pairs_mt560)
- **Access:** Hugging Face parquet; column `som`
- **Output:** `data/raw/mt560/mt560_so.jsonl`
- **License:** CC-BY-4.0 (verify on upstream)

### `quran`

- **Upstream:** [QuranEnc Somali (Yacob Yusuf)](https://quranenc.com/api/v1/translation/sura/somali_yacob/1) translation API
- **Access:** Direct HTTP JSON; suras 1–114 fetched concurrently; fields `translation` + `footnotes`
- **Output:** `data/raw/quran/translation.json` (verse text) and `data/raw/quran/footnotes.json` (footnote explanations)
- **Cleaning:** strips leading verse numbers and inline footnote markers from translations; strips leading `[n].` markers from footnotes; empty footnotes dropped
- **License:** see upstream QuranEnc terms (verify before release)

---

## Output paths

| Stage | Path pattern |
|-------|--------------|
| Per-source raw | `data/raw/<key>/<key>_so.jsonl` |
| Merged raw | `data/merged/merged_so.jsonl` |
| Cleaned | `data/cleaned/` (planned) |
| Deduplicated | `data/deduplicated/` (planned) |
| Filtered | `data/filtered/` (planned) |
| Final | `data/final/` (planned) |
| Rejected (sidecar) | `data/<stage>/<stage>.rejected.jsonl` (planned) |

---

## Adding a new source

1. Choose a stable registry key (lowercase, no spaces).
2. Add a row to this table with license, scale estimate, and tool.
3. Implement downloader or collector.
4. Update [METADATA_SCHEMA.md](METADATA_SCHEMA.md) if new `meta` fields are needed.
5. Document overlap expectations with existing sources.
