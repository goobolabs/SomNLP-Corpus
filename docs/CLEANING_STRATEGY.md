# Post-Clean Audit & v0.2 Cleaning Strategy

**Status:** v0.2 full pipeline run complete (2026-07-07)  
**Audit date:** 2026-07-07  
**Corpus audited:** `data/final/final_so.jsonl` (v0.2 full run) — **1,668,080** documents · **528,853,952** words · **317** avg words/doc · **4.0 GB**  
**v0.1 baseline:** 1,774,891 docs · 591M words · 333 avg words/doc · 4.5 GB (no deep clean)

This document audits what the **v0.1** pipeline ([CLEANING_PLAN.md](CLEANING_PLAN.md)) produced on the full Somali corpus, catalogs residual defects, documents the **v0.2** deep-clean strategy and its measured outcomes, and records the post-run audit. It is a follow-up to `CLEANING_PLAN.md`, not a replacement: the plan specifies Phase 3; this document specifies the second-pass fixes, grounded in measured defect rates.

Companion docs: [DATA_PIPELINE.md](DATA_PIPELINE.md), [QUALITY_METADATA.md](QUALITY_METADATA.md).

Where a recommendation touches code, the owning module is named. Where it touches a knob, the `configs/pipeline.toml` key is named.

---

## Executive summary

Phase 3 (merge → clean → LID → near-dedup) produces a large, reproducible corpus at the documented scale. **A measurable tail of specific artifacts still survives** — web crawl noise, mixed-language boilerplate, broken escaping, and encoding residue — concentrated in HPLT, mC4, MADLAD, and CC100.

**Why current gates miss them:**

1. **Review flags do not reject** — `html_remnant`, `high_symbol_ratio`, and `mostly_numbers` are recorded but records stay `disposition = kept`.
2. **LID clips to the first 2,000 bytes** — Somali headlines pass; English/Swedish bodies and nav chrome survive.
3. **No boilerplate / URL / markup stripping** — conservative by design in v0.1.
4. **Source-specific export bugs** — MADLAD stores literal `\n`; OPUS retains escaped HTML closers.
5. **Mojibake repair is CP1252-only, ≤3 passes** — split-indicator and double-encoded forms survive.

**v0.2 results (2026-07-07 full run):** deep clean removed 32,059 documents and masked
URLs in 16.17% of kept records. Post-run audit confirms URL remnants dropped from 18.8%
to 0.07%, escaped `\n` from 10.5% to 0.01%, and boilerplate from 4.3% to 1.06%.
Foreign-language markers remain at 12.81% (v0.1: 13.4%) — segment-level LID catches
additional non-Somali blocks but mixed-language pages with Somali headers still survive.

**Verdict:** v0.2 deep clean delivered the targeted reductions on URL noise, escaped
newlines, and boilerplate. Foreign-language markers remain the largest residual class
(12.8%, down slightly from 13.4%). The corpus is release-ready pending Hugging Face
packaging.

---

## 1. Method

### 1.1 Pipeline funnel (v0.2 full run)

```text
merge       2,633,281 in → 2,329,800 kept   (exact dedup, 11.52% dropped)
clean       2,329,800 in → 2,225,791 kept   (4.46% rejected; 21,405 flagged review)
lid         2,225,791 in → 2,035,287 kept   (8.56% dropped: not_somali 190,375)
deep_clean  2,035,287 in → 2,003,228 kept   (1.58% dropped: 32,059 rejected)
near-dedup  2,003,228 in → 1,668,080 kept   (16.73% dropped: near_duplicate 335,148)
────────────────────────────────────────────────────────────────────────────────
FINAL (v0.2)  data/final/final_so.jsonl    1,668,080 docs · 528,853,952 words
```

**Deep-clean reject breakdown:** boilerplate 23,948 · not_somali 6,906 · too_long 1,060 ·
mostly_numbers 117 · html_remnant 23.

**v0.1 baseline (no deep clean):** near-dedup 2,035,287 → 1,774,891 kept (12.79% dropped,
260,396 near-duplicates). v0.2 removed 106,819 more documents overall (36.7% vs 32.6% of
raw input).

LID reject languages: English 106,399, Tagalog 45,182, Swahili 21,547, Indonesian 6,791 — almost all mC4 drops (188,860). Near-dedup removed most from HPLT; the v0.2 pass removed 74,752 more near-duplicates than v0.1 because text normalization and boilerplate stripping exposed additional MinHash collisions.

**36.7%** of raw documents were removed overall; the kept **63.3%** carries substantially less crawl noise than the v0.1 baseline.

### 1.2 Dual audit methodology

Two complementary measurements were merged into this document:

| Method | Scope | Tools | Use |
|--------|-------|-------|-----|
| **Spread sample** | Every 30th final record (~59.2k docs) | `jq` / `grep` / `awk`; text flattened to one line before match | Conservative per-document rates; tracks **cleaned → final** movement |
| **Full corpus scan** | All 1,668,080 records (v0.2) / 1,774,891 (v0.1) | Python pattern detectors | Corpus-wide magnitudes; per-source breakdown; export-bug detection |

**Important:** rates differ between methods because (a) detectors differ in breadth (e.g. full scan matches `.com` domains, spread sample may use stricter URL patterns), and (b) spread sample flattens newlines, which **under-counts** MADLAD literal `\n` issues. Treat spread-sample rates as conservative floors; full-scan rates as upper-bound magnitudes. v0.2 post-run audit in `reports/06_cleaning_audit.md` supersedes the v0.1 spread-sample figures in §3.1 for key defect classes.

### 1.3 Manual sampling

Random and targeted samples from `final_so.jsonl`, reject sidecars, and raw sources. Compared raw → final on 2,000–3,000 records per source (e.g. MADLAD escaped newlines: 99.9% in raw and final — unchanged by pipeline).

---

## 2. What v0.1 already does well (baseline — keep)

Do not regress these:

- **Exact dedup:** 11.5% removed at merge (17.4% within HPLT alone) + 42 post-clean exact recheck.
- **Mojibake repair:** CP1252 round-trip ([`clean/mojibake.rs`](../crates/corpus-pipeline/src/clean/mojibake.rs)) cut the dominant artifact family to ~0.07% survivors. Improve-only guard prevents damaging clean `é`/`ñ` text.
- **U+FFFD reject gate:** ratio > 0.5% drops heavily corrupted docs (5,148 rejects, mostly mC4).
- **Two-class length floors:** 25-word document / 5-word sentence floors behave as designed (`too_short` = 98,819 rejects).
- **LID on OCR garbage:** sampled rejects are genuine noise with low `lang_score`. Of LID-kept docs, **99.9%** score ≥ 0.8.
- **Near dedup:** removed 12.8% at final stage; also collapsed much HTML-bearing boilerplate (literal tags 0.83% → 0.15% from cleaned to final).
- **Reject sidecars + stats:** preserve for v0.2 tuning.

---

## 3. Findings: residual defects

### 3.1 Summary tables

**Spread sample (N≈59.2k, per-document, cleaned → final):**

| # | Defect | cleaned → **final** | Today | Proposed owner |
|---|--------|--------------------|-------|----------------|
| D | **URLs** | 5.7% → **5.03%** | kept | clean (mask) |
| E | **Email (PII)** | 5.8% → **5.44%** | kept | clean (mask) |
| F | Code-switch / foreign boilerplate | 1.0% → **0.52%** | mostly kept | LID (segment-level) |
| B | Stray U+FFFD (below reject ratio) | 0.27% → **0.28%** | kept, unflagged | clean/strip |
| C | Literal HTML tags | 0.83% → **0.15%** | flagged `HtmlRemnant` | clean (strip/reject) |
| A | Mojibake survivors | 0.11% → **0.07%** | passed through | clean/mojibake |
| — | script/php/style scaffolding | — → **0.02%** | flagged, kept | clean (reject) |
| G | HTML entities encoded | 0.02% → **0.01%** | negligible | (no action) |
| H | Short-doc tail (`< 40` words) | ~17% → **6.1%** | kept by design | policy |

**Union URL-or-email (spread sample): ~9.78%** — largest single unaddressed need in conservative measurement.

**Full corpus scan (all 1.77M, overlapping detectors):**

| Issue class | Records | % of corpus |
|-------------|--------:|------------:|
| URL / domain remnants | ~334,000 | 18.8% |
| Foreign-language markers (EN/Scandinavian) | ~237,000 | 13.4% |
| Literal escaped `\n` (MADLAD) | ~186,000 | 10.5% |
| Site boilerplate / nav chrome | ~77,000 | 4.3% |
| Pipe-separated navigation | ~59,000 | 3.3% |
| WordPress / truncation (`[…]`) | ~32,000 | 1.8% |
| Arabic script (3+ chars) | ~17,500 | 1.0% |
| HTML tags (unstripped) | ~3,600 | 0.2% |
| Residual mojibake | ~1,300 | 0.07% |
| Intra-doc duplicate sentences | ~3,800 | 0.2% |

Review flags on **kept** records: `html_remnant` 2,685 · `mostly_numbers` 306 · `high_symbol_ratio` 7.

**Structural notes:**

- Near-dedup collapsed templated HTML boilerplate for free.
- LID halved code-switch rate (1.0% → 0.52%) on monolingual foreign docs but **cannot see** Somali-header / English-body pages.
- CC100 and HPLT have **<0.2% LID rejection** vs mC4 **22.7%** — head-clip bias.

### 3.2 Root cause map by source

| Source | Kept docs | Dominant defects | Fix location |
|--------|----------:|------------------|--------------|
| **hplt** | 620,964 | URLs (~22%), foreign EN (~13%), boilerplate | Paragraph LID; boilerplate strip; URL mask |
| **mc4** | 605,623 | URLs (~25%), foreign EN (~21%), pipe nav (~7%) | Paragraph LID; boilerplate strip |
| **madlad** | 185,907 | **Escaped `\n` (~100%)**, URLs, foreign EN | **Export unescape (P0)** |
| **cc100** | 301,074 | Ellipsis/truncation, dup paragraphs, mojibake | Intra-doc dedup; extended mojibake |
| **mt560** | 49,197 | Mostly clean (parallel) | Extended mojibake only |
| **opus** | 12,126 | HTML escape suffixes | **Downloader strip (P0)** |

---

## 4. Defect catalog (detail)

### A. Mojibake survivors

~0.07% (spread) / ~1,284 (full scan). Two patterns:

1. **Un-repaired smart quotes:** `Masâ€™uuliyiinta … â€œWaa ayaan darro … laâ€™aan` — improve-only guard may reject valid repair when no other CP1252 content tips the comparison.
2. **Whitespace-split / multiply-encoded:** `vappupÃ ¤ ivÃ ¤`; deep nests like `daÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢deedu` exceeding 3-pass cap.

### B. Stray U+FFFD below reject threshold

~0.28%. Lone `�` in otherwise-clean Somali; ratio far under 0.005 so doc is kept unflagged.

### C. Literal HTML / script / PHP remnants

0.15% tags in final (down from 0.83% cleaned). v0.1 **flags but does not strip** (`HtmlRemnant`, 17,432 flagged at clean). Split populations:

- **Benign:** `</a>`, `<br>`, `<p>`, `<b>`
- **Junk:** `<?php`, `</script>`, `<mutation>`, `<char>`

### D & E. URLs and email addresses

Spread: ~5% URLs, ~5.4% emails, ~9.8% union. Full scan: ~18.8% URL-like (broader detector). Concentrated in mC4 ≫ HPLT > MADLAD; CC100 almost none. Emails are **PII** — mask recommended for pretraining release.

### F. Code-switching / non-Somali blocks

Document-level LID clips first 2000 bytes ([`lid/stage.rs`](../crates/corpus-pipeline/src/lid/stage.rs)). Failure modes: wholly mislabeled mC4 `so` pages; Somali header + English/Swedish body (e.g. `La daabacay måndag 16 februari 2015`).

### G. HTML entities

~0.01% survivors. [`clean/entities.rs`](../crates/corpus-pipeline/src/clean/entities.rs) works; no action needed.

### H. Short-document tail

~6.1% of kept docs `< 40` words; by design (25-word floor benchmark-backed). Policy decision only — see §6.6.

### I. Cross-document vs intra-document boilerplate

Near-dedup **completed** — templated footers largely collapsed. **Intra-document** line repetition is low (~0.22% duplicate sentences full scan) but CC100 syndication duplicates matter. Residual cross-doc boilerplate: re-probe if templated footers remain >1% after v0.2.

### J. Literal escape sequences — MADLAD (P0 export bug)

**~186,083 records (10.5%); 99.98% of MADLAD kept docs.**

```text
Madaxweyneyaasha Somaaliya Iyo Kenya ...\nMarch 6, 2019 - Written by C M\nNairobi-(GLN): ...
```

**Root cause:** MADLAD JSONL.gz escaped strings written verbatim ([`export_json_gz_shards`](../crates/corpus-tools/src/export.rs)). Clean stage does not unescape.

### K. OPUS HTML escape residue (P0 export bug)

Many records with `<\/a><\/blockquote>\n` suffixes from ParaCrawl `translation.so`. Fix at download or pre-clean.

### L. Site boilerplate and navigation (full scan)

Boilerplate markers: 77,113 (4.3%). Pipe nav: 59,154 (3.3%). WordPress: 18,239 (1.0%). Example:

```text
HOMEGABAYODUCOOYINQURAAN... CONTACT US
Live Help/Live Chat
Radio Saajid 682-710-3731 »
```

### M. Arabic / non-Latin script

17,498 records (0.99%) with Arabic runs — often legitimate (Qur'an citations). Tag, do not blanket-remove; reject only if Arabic ratio >40% and LID ≠ Somali.

### N. Smart quotes (typographic punctuation)

52% contain U+2018/U+2019/U+201C/U+201D — mostly valid post-repair. Low priority unless tokenizer analysis shows harm.

---

## 5. v0.2 strategy (implemented)

Every step keeps the pipeline contract: stream where possible, route removals to sidecars with reasons, write before/after counts to stage reports.

### 5.1 Pipeline overview

Integrated into the main pipeline (v0.2):

```text
data/lid/lid_so.jsonl
    → deep_clean (4a–4f)     data/deep_clean/deep_clean_so.jsonl
    → near_dedup (4g)        data/final/final_so.jsonl
```

Sub-steps inside `deep_clean`:

```text
4a. Source-aware normalize   (MADLAD/OPUS unescape, extended mojibake)
4b. Markup & contact clean   (HTML tiering, URL/email mask)
4c. Boilerplate removal       (line classifier, nav patterns)
4d. Language purity           (segment-level LID)
4e. Intra-document dedup      (duplicate paragraphs)
4f. Quality heuristics v2     (promote review flags → reject)
4g. Near-dedup                (MinHash on changed text → final release)
```

Optional later (Track B): **4h. Char-n-gram quality filter** (Wikipedia-so seed).

### 5.2 Implementation priorities

| Priority | Item | Owner module | Effort | Est. impact |
|:--------:|------|--------------|--------|-------------|
| **P0** | MADLAD literal unescape | `corpus-tools/export.rs` or `clean/normalize` | Low | ~186,000 docs |
| **P0** | OPUS HTML escape strip | `download_opus_so` or `clean` | Low | ~12,000 docs |
| **P1** | Extended mojibake (split indicators, extra passes) | `clean/mojibake.rs` | Medium | ~1k–50k |
| **P1** | Strip stray U+FFFD from kept docs | `clean/strip.rs` | Low | ~0.28% |
| **P1** | HTML two-tier (reject scaffolding, strip benign tags) | new `clean/html.rs` | Medium | ~3.6k+ |
| **P1** | Segment-level LID | `lid/stage.rs` | Medium | 100k–300k |
| **P1** | Boilerplate line removal | new `clean/boilerplate.rs` | Medium | ~75k–150k |
| **P2** | URL/email mask (`⟨url⟩` / `⟨email⟩`) | new `clean/contact.rs` | Low | ~10–19% union |
| **P2** | Intra-doc paragraph dedup | new `clean/intra_dedup.rs` | Medium | ~4k–30k |
| **P2** | Promote review flags → reject | `clean/stage.rs` | Low | ~3k |
| **P3** | Char-n-gram quality filter | deferred | High | TBD |
| **P3** | Main-content extraction (trafilatura) | future web sources | High | Track B |

### 5.3 Priority detail (code-anchored)

**P0 — Export / source-aware normalize (findings J, K)**

- MADLAD: unescape `\n`, `\t`, `\"`, `\\/`; re-run whitespace normalize.
- OPUS: strip trailing `<\/?…>` fragments; decode JSON escapes before clean chain.

**P1 — Extended mojibake (finding A)**

- Pre-pass: rejoin whitespace-split indicators (`Ã ¤` → `Ã¤`, `â€ ™` → `â€™`) when continuation is known.
- Then existing improve-only CP1252 round-trip (optionally raise max passes to 5).
- Golden set: extend [`tests/mojibake_golden.rs`](../crates/corpus-pipeline/tests/mojibake_golden.rs) with real survivor lines.

**P1 — Strip stray U+FFFD (finding B)**

- In `clean/strip.rs`, drop isolated U+FFFD **after** mojibake and **after** ratio gate.

**P1 — HTML two-tier policy (finding C)**

- **Tier 1 reject:** `<?php`, `<script>…</script>`, `<style>…</style>`, `<mutation>`, `<char>`.
- **Tier 2 strip-keep:** benign inline tags; preserve inner text; re-normalize whitespace.
- Keep `HtmlRemnant` flag on residue after strip.

**P2 — URL/email handling (findings D, E)**

Recommended for pretraining: **mask, don't drop.**

- URLs → `⟨url⟩` sentinel; emails → `⟨email⟩` (PII).
- Alternatives: strip token (fragments sentences) or flag-only (defer). Record choice in changelog.

**P1 — Segment-level LID (finding F)**

- Per-paragraph LID; reject whole doc if Somali segment fraction < 0.6–0.7 (tune on FLORES-200).
- **Or** sample LID at head + middle + tail (3 × 1 KB) instead of head-only 2 KB.
- Option to drop non-Somali segments (higher risk — prototype first).
- Sentence-class (OPUS, MT560): unchanged tag-only.

**P1 — Boilerplate removal (finding L)**

Line-drop rules (illustrative):

```text
DROP line if:
  - all-caps nav run (^[A-Z0-9|«»]{10,}$)
  - pipe menu (|foo|bar|)
  - (Written by|Posted by|Tags:|CLICK HERE|CONTACT US|Live Help)
  - phone-only line
  - < 4 words and no Somali function word
```

Reject doc if >40% lines dropped as boilerplate or below length floor.

**P2 — Intra-document dedup (finding I, G-truncation)**

- Remove exact/near-duplicate consecutive paragraphs (Jaccard ≥ 0.95).
- Flag/reject WordPress `[…]` truncation patterns.

**P2 — Quality heuristics v2**

| Flag | v0.1 | v0.2 proposed |
|------|------|---------------|
| `html_remnant` | review | reject if tags remain after strip |
| `high_symbol_ratio` | review @ >0.5 | reject @ >0.45; review @ >0.35 |
| `mostly_numbers` | review | reject (document class) |
| (new) | — | reject if >10,000 words |

**P1 — Re-near-dedup (4g)**

Re-run MinHash + LSH (τ=0.80) on document class after text changes.

### 5.4 Outcomes (v0.2 actual vs v0.1 baseline)

| Metric | v0.1 | v0.2 actual |
|--------|-----:|------------:|
| Documents | 1,774,891 | **1,668,080** |
| Words | 591M | **529M** |
| File size | 4.5 GB | **4.0 GB** |
| URL noise (full scan) | 18.8% | **0.07%** |
| Escaped `\n` | 10.5% | **0.01%** |
| URL sentinel masked (`⟨url⟩`) | — | **16.17%** |
| Foreign EN markers | 13.4% | **12.81%** |
| Boilerplate markers | 4.3% | **1.06%** |

Near-dedup removal increased from 260,396 (v0.1) to 335,148 (v0.2) because deep-clean
normalization exposed additional near-duplicate clusters.

---

## 6. Configuration

Unified v0.2 knobs (opt-in defaults preserving v0.1 behavior):

```toml
[clean]
# v0.2 additions
strip_benign_html   = true        # Tier-2: remove inline tags, keep inner text
reject_script_html  = true        # Tier-1: reject <?php>/<script>/<style>/<mutation>
strip_stray_ufffd   = true        # drop lone U+FFFD from kept docs
mask_urls           = true        # URLs → ⟨url⟩ sentinel
mask_emails         = true        # emails → ⟨email⟩ sentinel (PII)

[lid]
# v0.2 additions
segment_level        = true       # per-paragraph LID in addition to doc-level
min_somali_char_frac = 0.6        # reject doc if Somali segment fraction below this
# Alternative: multi-offset clip instead of segment_level

[deep_clean]
unescape_madlad = true
strip_opus_html = true
mojibake_max_passes = 5
boilerplate_line_drop = true
boilerplate_reject_ratio = 0.40

[deep_clean.intra_dedup]
enabled = true
paragraph_jaccard_tau = 0.95

[deep_clean.heuristics]
symbol_ratio_reject = 0.45
symbol_ratio_review = 0.35
max_document_words = 10000
```

---

## 7. Validation plan

1. **Golden tests** — mojibake split-indicators, HTML tiering, MADLAD/OPUS unescape ([`crates/corpus-pipeline/tests/`](../crates/corpus-pipeline/tests/)).
2. **Sidecar diffing** — 50 sampled rejects per new reason; confirm none are good Somali prose.
3. **Dual re-measurement** — re-run spread-sample probes **and** full-corpus detectors; recorded in `reports/06_cleaning_audit.md`. Results: URL remnants 18.8% → 0.07%, escaped `\n` 10.5% → 0.01%, boilerplate 4.3% → 1.06%.
4. **LID no-regression** — FLORES-200 Somali recall must not drop.
5. **Human review** — 20 records/source from `data/final/final_so.jsonl` after v0.2 run; score 1–5 on readability, purity, boilerplate, encoding (target median ≥4.0 kept).

---

## 8. Publishing implications (Hugging Face)

Do **not** publish the v0.1 corpus for a pretraining-focused release.

| Release | Contents | When |
|---------|----------|------|
| v0.1-preview | Dataset card + 10K sample + pipeline recipe | Optional |
| **v0.2-clean** | Full `data/final/final_so.jsonl` after deep clean + near-dedup | **Ready** (HF packaging pending) |
| splits | train 99% / val 1% stratified by source | At publish |

Document: v0.1 vs v0.2 differences, residual noise estimates, per-source licenses ([SOURCES.md](SOURCES.md)), reproduction commands.

Target repo: `goobolabs/SomNLP-Corpus`.

---

## 9. Concrete examples (from final corpus)

**Swedish + English in Somali news (passes LID):**
```text
Ninka Fadeexada badan ee Erik Almqvist oo xukuma warbaahinta SD - Radio Sweden Somali ...
La daabacay måndag 16 februari 2015 kl 12.30
```

**Product spec spam (HPLT):**
```text
Tags:
Black ama Silver? The Miisaanka Digital Jikada Cuntada Pro Ozeri looks the part, laakiin ...
- Range: 1g – 5000g
```

**Double-encoded mojibake (CC100):**
```text
... suÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢aalaha la weydiinaayeyna ay ku saabsanaayeen habka ku qanacsanaanta guurka ...
```

**WordPress truncation (CC100):**
```text
... Sidoo […] Madaxda Wasaaradda ... […] Wasiirka Cusub ee Was ...
```

**Navigation chrome (mC4):**
```text
HOMEGABAYODUCOOYINQURAAN... CONTACT US
Live Help/Live Chat
Radio Saajid 682-710-3731 »
```

---

## 10. Open questions

1. URL/email: mask vs strip vs flag — **recommended mask**; needs sign-off.
2. HTML Tier-1 reject token list — complete, or data-driven from full `HtmlRemnant` sidecar?
3. Segment LID: drop-segment vs reject-document — prototype both.
4. Promote `MostlyNumbers`/`HighSymbolRatio` to reject before char-n-gram filter, or hold?
5. Smart quotes: keep Unicode or NFKC to ASCII for tokenizer efficiency?

---

## 11. Next steps

1. ~~Review and lock priority order / rejection aggressiveness.~~ (implemented in code)
2. ~~Implement **P0** in `corpus-tools` (MADLAD unescape, OPUS strip).~~
3. ~~Implement v0.2 stages in `corpus-pipeline` (`deep_clean` binary).~~
4. ~~**Run full pipeline** — `run_pipeline` from merge through near-dedup; v0.2 funnel
   recorded in §1.1 (`reports/04_deep_clean_stats.json`, `reports/05_near_dedup_stats.json`).~~
5. ~~Dual audit re-measurement; compare to §3.1 tables (`reports/06_cleaning_audit.md`).~~
6. Publish v0.2 to Hugging Face.

---

## Appendix A — Full-corpus detector summary

**v0.1 baseline (2026-07-07, 1,774,891 docs):**

| Detector | Count | % |
|----------|------:|--:|
| smart_quote_artifact | 927,328 | 52.25% |
| url | 333,566 | 18.79% |
| foreign_en | 236,967 | 13.35% |
| escaped | 186,083 | 10.48% |
| boilerplate | 77,113 | 4.34% |
| pipe_nav | 59,154 | 3.33% |
| ellipsis_trunc | 31,914 | 1.80% |
| wordpress | 18,239 | 1.03% |
| arabic | 17,498 | 0.99% |
| dup_sentence | 3,843 | 0.22% |
| html_tag | 3,602 | 0.20% |
| mojibake | 1,284 | 0.07% |
| nbsp_artifact | 711 | 0.04% |
| php_code | 350 | 0.02% |

**v0.2 post-run audit (`reports/06_cleaning_audit.md`, 1,668,080 docs):**

| Detector | v0.1 % | v0.2 % | Change |
|----------|-------:|-------:|--------|
| url (raw) | 18.79% | **0.07%** | −18.7 pp |
| escaped `\n` | 10.48% | **0.01%** | −10.5 pp |
| sentinel_url (masked) | — | **16.17%** | new |
| foreign_en | 13.35% | **12.81%** | −0.5 pp |
| boilerplate | 4.34% | **1.06%** | −3.3 pp |

## Appendix B — Per-source issue rates (full scan)

| Source | escaped | url | foreign_en | boilerplate | pipe_nav |
|--------|--------:|----:|-----------:|------------:|---------:|
| hplt | 0.0% | 22.2% | 12.9% | 3.3% | 0.8% |
| mc4 | 0.0% | 24.8% | 20.5% | 7.8% | 7.0% |
| madlad | **99.98%** | 16.6% | 15.1% | 5.0% | 3.8% |
| cc100 | 0.0% | 4.8% | 1.5% | 0.0% | — |
| mt560 | ~0% | ~0% | ~0% | — | — |
| opus | 0.07% | 0.9% | 0.8% | — | — |

## Appendix C — References

- Phase 3 spec: [CLEANING_PLAN.md](CLEANING_PLAN.md)
- Pipeline commands: [DATA_PIPELINE.md](DATA_PIPELINE.md)
- LID benchmark: `reports/lid_benchmark.md`
- Min-word benchmark: `reports/min_word_threshold_benchmark.md`
- Stage stats: `reports/01_merge_stats.md` … `reports/05_near_dedup_stats.md`
- v0.2 cleaning audit: `reports/06_cleaning_audit.md`
