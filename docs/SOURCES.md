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
| `quran` | QuranEnc Somali (Yacob Yusuf) | religious | A | 7,373 rows (6,236 verses + 1,137 footnotes) | see source | `download_quran_so` | done | Two outputs: verse translations and footnote explanations |
| `wikipedia` | Somali Wikipedia | encyclopedia | A | 9,021 articles (measured) | CC-BY-SA-4.0 | `download_wikipedia_so` | done | HF `wikimedia/wikipedia` dataset (`20231101.so`) |
| `xlsum` | XL-Sum Somali | news / summary | A | 7,452 articles (measured) | CC-BY-4.0 | `download_xlsum_so` | done | `csebuetnlp/xlsum` (`somali` config) |
| `nllb` | NLLB English–Somali | parallel text | A | 10.2M pairs raw · 4.7M kept at merge | ODC-BY | `download_nllb_so` | done | Official Meta/AllenAI NLLB TSV export |
| `glot` | Glot500 Somali (`som_Latn`) | web crawl / aggregated | A | ~3.9M docs (HF train) | see source | `download_glot_so` | done | HF `cis-lmu/Glot500` parquet export |
| `somali-web-corpus` | Somali Web Corpus V1 | web crawl | A | ~218K docs (HF train) | MIT | `download_somali_web_corpus_so` | done | HF `maanka2/somali-web-corpus` parquet export |
| `finepdfs` | FinePDFs Somali (`som_Latn`) | PDF documents | A | 21,781 docs (HF train) | ODC-BY | `download_finepdfs_so` | done | HF `HuggingFaceFW/finepdfs` parquet export; long documents |
| `fineweb-2` | FineWeb-2 Somali (`som_Latn`) | web crawl | A | 1,070,384 docs (HF train) | ODC-BY | `download_fineweb2_so` | done | HF `HuggingFaceFW/fineweb-2` parquet export; heavy overlap with HPLT/mC4 |
| `somali-dataset` | Somali Alpaca instructions | instruction data | A | 44,839 rows (HF train) | MIT | `download_somali_dataset_so` | done | HF `burtugeey/Somali_dataset`; instruction, input, and output joined per row |
| `somali-tinystories` | Somali TinyStories | synthetic stories | A | 42,000 rows (HF train) | unspecified | `download_somali_tinystories_so` | done | HF `Zyroxx66/somali-tinystories`; no license on dataset card; 20,900 stories duplicated upstream |
| `tanzil` | Tanzil Qur'an Somali (Abduh) | religious | A | 6,236 ayahs | see source | `download_quran_tanzil` | done | Separate translation from QuranEnc; no footnotes upstream |

### Track A licensing note

There is **no single corpus license**. Redistribution of the combined corpus requires
honoring each upstream license. The `license` field on processed records is copied
from this registry (see [METADATA_SCHEMA.md](METADATA_SCHEMA.md)).

### Track A outlook

Seventeen public sources total **18.2M raw documents** (NLLB dominates). The full
seventeen-source Track A corpus measured **7,981,982 final documents · 832M words ·
1,136M v2 tokens** (2026-09-24 run — see [README.md](../README.md)). The previous
thirteen-source run (2026-09-02) measured 7.35M final documents · 666M words · 912M v2 tokens. Incremental measurement
notes: [reports/runs/MEASUREMENT.md](../reports/runs/MEASUREMENT.md).

### Measured final documents (2026-09-24)

| Key | Raw | Kept at merge | Final | Words | v2 tokens |
|-----|----:|-------------:|------:|------:|----------:|
| `nllb` | 10,229,073 | 4,372,863 | 4,108,233 | 62,072,423 | 82,095,163 |
| `glot` | 3,915,898 | 3,840,628 | 1,432,277 | 66,937,207 | 89,396,283 |
| `fineweb-2` | 1,070,384 | 854,429 | 589,824 | 156,995,286 | 208,794,537 |
| `mc4` | 893,012 | 892,852 | 586,265 | 214,035,586 | 313,609,489 |
| `hplt` | 966,507 | 798,161 | 554,851 | 184,700,703 | 247,457,800 |
| `cc100` | 396,524 | 374,720 | 296,154 | 49,297,018 | 61,321,488 |
| `madlad` | 200,494 | 177,030 | 128,700 | 61,831,676 | 82,255,318 |
| `somali-web-corpus` | 217,528 | 217,390 | 121,764 | 4,890,563 | 6,111,014 |
| `mt560` | 161,865 | 51,083 | 49,195 | 1,162,745 | 1,423,242 |
| `somali-dataset` | 44,839 | 44,793 | 37,512 | 5,781,223 | 7,974,652 |
| `somali-tinystories` | 42,000 | 21,100 | 21,100 | 2,396,895 | 3,074,429 |
| `finepdfs` | 21,781 | 21,780 | 20,771 | 18,625,820 | 27,654,658 |
| `opus` | 14,879 | 12,296 | 12,126 | 381,716 | 517,056 |
| `quran` | 7,373 | 7,277 | 7,072 | 160,068 | 240,053 |
| `quran-tanzil` | 6,236 | 6,048 | 5,773 | 106,282 | 164,038 |
| `wikipedia` | 9,021 | 8,994 | 5,338 | 1,130,331 | 1,748,915 |
| `xlsum` | 7,452 | 7,451 | 5,027 | 1,758,807 | 2,189,823 |
| **Total** | **18,204,866** | **11,708,895** | **7,981,982** | **832,264,349** | **1,136,027,958** |

Raw, merge, and final counts come from `reports/01_merge_stats.json` and
`reports/05_near_dedup_stats.json`; words and tokens from `tokenizer/corpus_token_stats.json`
(which skips the 6 empty records).

### Measured final documents (2026-08-31)

| Key | Raw | Kept at merge | Final |
|-----|----:|-------------:|------:|
| `nllb` | 10,229,073 | 4,727,602 | 4,463,021 |
| `mc4` | 893,012 | 892,852 | 595,193 |
| `hplt` | 966,507 | 798,364 | 576,995 |
| `cc100` | 396,524 | 374,721 | 300,664 |
| `madlad` | 200,494 | 200,484 | 133,410 |
| `mt560` | 161,865 | 51,083 | 49,195 |
| `opus` | 14,879 | 12,296 | 12,126 |
| `quran` | 7,373 | 7,277 | 7,072 |
| `xlsum` | 7,452 | 7,451 | 5,649 |
| `wikipedia` | 9,021 | 8,994 | 5,503 |
| `quran-tanzil` | 6,236 | 6,048 | 5,773 |
| **Total** | **12,892,436** | **7,087,172** | **6,154,601** |

---

## Registry — Track B (collected sources)

| Key | Name | Category | Track | Est. scale | License | Tool | Status | Notes |
|-----|------|----------|:-----:|------------|---------|------|--------|-------|
| `web` | Somali web scraping | news / blogs / gov | B | ~40M words | per-site | collector (planned) | planned | ~100 sites; robots.txt required |
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

### `wikipedia`

- **Upstream:** [wikimedia/wikipedia](https://huggingface.co/datasets/wikimedia/wikipedia) config `20231101.so`
- **Access:** Hugging Face parquet shards
- **Output:** `data/raw/wikipedia/wikipedia_so.jsonl`
- **License:** CC-BY-SA-4.0

### `xlsum`

- **Upstream:** [csebuetnlp/xlsum](https://huggingface.co/datasets/csebuetnlp/xlsum) config `somali`
- **Access:** Hugging Face parquet auto-converted revision (`PARQUET_REVISION`)
- **Output:** `data/raw/xlsum/xlsum_so.jsonl`
- **License:** CC-BY-4.0

### `glot`

- **Upstream:** [cis-lmu/Glot500](https://huggingface.co/datasets/cis-lmu/Glot500) config `som_Latn`
- **Access:** Hugging Face parquet export (`refs/convert/parquet`); train split
- **Output:** `data/raw/glot/glot_so.jsonl`
- **License:** see upstream Glot500 dataset card

### `somali-web-corpus`

- **Upstream:** [maanka2/somali-web-corpus](https://huggingface.co/datasets/maanka2/somali-web-corpus) config `default`
- **Access:** Hugging Face parquet export (`refs/convert/parquet`); train split (~218K rows)
- **Output:** `data/raw/somali-web-corpus/somali-web-corpus_so.jsonl`
- **License:** MIT

### `finepdfs`

- **Upstream:** [HuggingFaceFW/finepdfs](https://huggingface.co/datasets/HuggingFaceFW/finepdfs) subset `som_Latn`
- **Access:** Hugging Face parquet export (`refs/convert/parquet`); `som_Latn/train` shards, `text` column
- **Output:** `data/raw/finepdfs/finepdfs_so.jsonl`
- **License:** ODC-BY

### `fineweb-2`

- **Upstream:** [HuggingFaceFW/fineweb-2](https://huggingface.co/datasets/HuggingFaceFW/fineweb-2) subset `som_Latn`
- **Access:** Hugging Face parquet export (`refs/convert/parquet`); `som_Latn/train` shards, `text` column
- **Output:** `data/raw/fineweb-2/fineweb-2_so.jsonl`
- **License:** ODC-BY
- **Overlap:** built from Common Crawl like HPLT and mC4; 215,950 rows are exact
  cross-source duplicates at merge and 235,557 more are removed by near-dedup (2026-09-24)

### `somali-dataset`

- **Upstream:** [burtugeey/Somali_dataset](https://huggingface.co/datasets/burtugeey/Somali_dataset) config `default`
- **Access:** Hugging Face parquet export (`refs/convert/parquet`); train split. Each row's
  non-empty `instruction`, `input`, and `output` are joined with a blank line
- **Output:** `data/raw/somali-dataset/somali-dataset_so.jsonl`
- **License:** MIT (dataset card)

### `somali-tinystories`

- **Upstream:** [Zyroxx66/somali-tinystories](https://huggingface.co/datasets/Zyroxx66/somali-tinystories) config `default`
- **Access:** Hugging Face parquet export (`refs/convert/parquet`); train split, `story` column
- **Output:** `data/raw/somali-tinystories/somali-tinystories_so.jsonl`
- **License:** not stated on the dataset card; recorded as `Other`. Confirm with the owner
  before redistribution
- **Data note:** 20,900 of the 42,000 upstream stories appear twice verbatim; merge keeps one copy

### `nllb`

- **Upstream:** [allenai/nllb](https://storage.googleapis.com/allennlp-data-bucket/nllb/eng_Latn-som_Latn.gz)
- **Access:** Direct Google Cloud Storage bucket gzip stream
- **Output:** `data/raw/nllb/nllb_so.jsonl`
- **License:** ODC-BY

### `tanzil`

- **Upstream:** [Tanzil.net](https://tanzil.net/trans/?transID=so.abduh&type=txt-2&agree=true) (Mahmud Muhammad Abduh translation)
- **Access:** Direct HTTP TXT download; 6,236 ayahs parsed
- **Output:** `data/raw/quran-tanzil/translation.json`
- **License:** see upstream Tanzil.net terms

---

## Output paths

| Stage | Path pattern |
|-------|--------------|
| Per-source raw | `data/raw/<key>/<key>_so.jsonl` |
| Merged raw | `data/merged/merged_so.jsonl` |
| Cleaned | `data/cleaned/cleaned_so.jsonl` |
| LID verified | `data/lid/lid_so.jsonl` |
| Deep clean (v0.2) | `data/deep_clean/deep_clean_so.jsonl` |
| Final (near dedup) | `data/final/final_so.jsonl` |
| Rejected (sidecar) | `data/<stage>/*.rejected.jsonl` |

---

## Adding a new source

1. Choose a stable registry key (lowercase, no spaces).
2. Add a row to this table with license, scale estimate, and tool.
3. Implement downloader or collector.
4. Update [METADATA_SCHEMA.md](METADATA_SCHEMA.md) if new `meta` fields are needed.
5. Document overlap expectations with existing sources.
