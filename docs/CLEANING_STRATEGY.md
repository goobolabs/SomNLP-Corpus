# Post-Clean Audit & v0.2 Cleaning Strategy

Status: research / proposal. Not yet locked.
Author basis: empirical audit of the v0.1 pipeline output, 2026-07-07.

This document audits what the **v0.1** pipeline (`docs/CLEANING_PLAN.md`) actually
produced on the full Somali corpus, catalogs the residual defects that survive into
`data/cleaned/` (and, where measurable, `data/lid/`), and proposes a prioritized
**v0.2** cleaning strategy. It is a follow-up to `CLEANING_PLAN.md`, not a replacement:
the plan specifies the pipeline; this document specifies the *next* round of fixes,
grounded in measured defect rates rather than expectation.

Where a recommendation touches code, the owning module is named so the change has a
home. Where it touches a knob, the `configs/pipeline.toml` key is named.

---

## 1. Method

The run under audit:

```text
merge  2,633,281 in → 2,329,800 kept   (exact dedup, 11.52% dropped)
clean  2,329,800 in → 2,225,791 kept   (4.46% rejected; 21,405 flagged for review)
lid    running at audit time — output partial (mt560, opus, cc100, mc4 processed;
       madlad, hplt not yet). Post-LID rates below are from this partial output.
near-dedup  pending
```

Sampling for defect measurement:

- **Cleaned corpus (all 6 sources):** a spread sample of every 40th record across
  the full `data/cleaned/cleaned_so.jsonl` (~55.6k docs), plus a per-document
  single-pass measurement on every-4th-of-spread (~13.9k docs). "Per-document" means
  *the document contains at least one occurrence*; each doc's text was flattened
  (newlines/tabs → space) before matching so multi-line documents count once.
- **LID-kept output (partial):** every 20th record of `data/lid/lid_so.jsonl`
  (~29.6k docs, only the four sources processed so far).

All probe commands are shell one-liners over `jq`/`grep`/`awk`; rates are reproducible
from the sampled files. Numbers below are rounded; treat them as magnitudes, not
exact corpus-wide constants.

---

## 2. What v0.1 already does well (baseline — keep)

The audit confirms the pipeline is sound and most cleaning goals are met. Do not
regress these:

- **Exact dedup is doing real work:** 11.5% removed at merge (17.4% inside HPLT
  alone), plus a post-clean exact recheck (42 more).
- **Mojibake repair fires and holds:** the CP1252 round-trip
  (`clean/mojibake.rs`) reduced the dominant artifact family to a **0.11%**
  per-document survivor rate in cleaned output. Guards (indicator-gated, improve-only,
  ≤3 passes) prevented over-correction — clean `é`/`ñ` text was not damaged.
- **U+FFFD reject gate works:** heavily corrupted docs (ratio > 0.5%) are dropped;
  `corrupted` accounts for 5,148 rejects, concentrated in mC4 (5,004).
- **Two-class length floors behave as designed:** the `too_short` reason (98,819,
  95% of all rejects) is dominated by CC100/mC4 short web fragments; HPLT (pre-filtered
  upstream) and the sentence-class sources barely trip it.
- **LID is catching OCR garbage cleanly:** sampled LID rejects are exactly the
  failure mode we want gone — scanned-page noise like `oo oo co ^ 2 . - - to CTl`
  with `lang_score` in the 0.1–0.3 range. Of LID-*kept* docs, **99.9%** score ≥ 0.8,
  so the 0.50 threshold is rarely the deciding factor; the kept set is high-confidence
  Somali.

The verdict is not "the pipeline is broken." It is "a measurable tail of specific
artifacts survives, and each one now has enough data to be handled deliberately."

---

## 3. Findings: residual defects

Per-document rates on the cleaned spread sample (all 6 sources). "Owner" is the stage
that should fix it; "v0.1 disposition" is what happens today.

| # | Defect | Rate (cleaned) | v0.1 disposition | Proposed owner |
|---|--------|---------------|------------------|----------------|
| A | Mojibake survivors (`Ã`, `â€`) | 0.11% | passed through | clean/mojibake |
| B | Stray U+FFFD (below reject ratio) | 0.27% | kept, unflagged | clean/gates |
| C | Literal HTML/script/PHP tags | 0.83% | flagged `HtmlRemnant`, kept | clean (new step) |
| D | URLs | ~5.7% | kept as-is | clean (new step) |
| E | Email addresses | ~5.8% | kept as-is | clean (new step) |
| F | Code-switch / non-Somali blocks | ~1.0% (0.38% post-LID) | mostly kept | LID (segment-level) |
| G | HTML entities left encoded | 0.02% | negligible | (no action) |
| H | Short-doc tail (`< 40` words) | ~17% kept | kept by design | policy decision |
| I | Cross-document boilerplate / near-dups | not yet measured | near-dedup pending | near-dedup |

### A. Mojibake survivors — indicator-splitting gap

0.11% of cleaned docs still contain `Ã`/`â€`. Inspection shows the survivors are not
random; the dominant pattern is **whitespace-split indicators**, e.g.:

```text
maalinta shaqaalaha ( vappupÃ ¤ ivÃ ¤ )      # should be "vappupäivä"
```

The repair in `clean/mojibake.rs` gates on contiguous indicator strings
(`"Ã"`, `"â€"`, `"Ã¤"`, …). When the source inserted a space or tag boundary between
the lead byte and the continuation (`Ã ¤` instead of `Ã¤`), the round-trip either
never fires or cannot reassemble the pair, and the artifact survives. Some survivors
also co-occur with `<?php`/`</a>` fragments, i.e. the mojibake sits inside HTML noise
that itself should go (finding C).

### B. Stray U+FFFD below the reject threshold

0.27% of cleaned docs carry a lone `�`. These are *not* the corrupted docs the 0.5%
ratio gate targets — they are otherwise-clean Somali sentences with a single
irrecoverable character, e.g.:

```text
cc100 | Haweeneydii carruurtaasi dhashay ayaa iyadu sheegtay iney taleefan kula … �
```

At one replacement char per long document the ratio sits far under 0.005, so the doc
is kept and the `�` is neither removed nor flagged. Low severity, but it is visible
noise in the final text and trivial to address.

### C. Literal HTML / script / PHP remnants

0.83% of cleaned docs contain literal tags. v0.1 deliberately **flags but does not
strip** these (`HtmlRemnant`, 17,432 flagged corpus-wide) — the plan calls tag
stripping "where conservative cleaning turns destructive" and defers it. The audit
shows the flagged content splits into two populations:

- **Benign inline tags** in otherwise-good Somali text: `</a>`, `<br>`, `<b>`, `<p>`.
- **Genuine junk** that should not be in a text corpus at all: `<?php`, `</script>`,
  `<div class="recruiter-section">`, `<mutation>`, `<char>`. Example:

```text
Su-aalo Diini Ah · Hidaha iyo Dhaqanka ... <?php </a> Kismaayo- Shirka Golaha …
```

The blanket "flag, never strip" rule is now too coarse: it leaves script/markup
scaffolding embedded in kept documents.

### D & E. URLs and email addresses

~5.7% of cleaned docs contain a URL; ~5.8% contain an email address. Neither is
touched by any stage today. URLs concentrate in the web-crawl sources (mC4 ≫ HPLT >
MADLAD; CC100 almost none), consistent with crawl boilerplate — navigation, "share
this", contact footers:

```text
Puh : 029 512 000            # phone/contact fragments (also trip mostly_numbers)
Tel . 045 639 6274
www.aylaseven.net Favicon 12a3 …
```

For a **pretraining** corpus a bare URL or email token is low-value and mildly
privacy-sensitive (emails are PII). This is a policy call, not an obvious bug — see §4.D.

### F. Code-switching / non-Somali blocks survive document-level LID

The document-level LID gate clips to the first 2000 bytes (`detect_clip_bytes`) and
accepts the doc if the winner is Somali. Two failure modes leak through:

1. **Mislabeled monolingual docs** in mC4's `so` split that are wholly another
   language (English, Oromo, Telugu song-lyric pages, Urdu/Arabic Quran-lesson pages).
2. **Code-switched docs** where Somali dominates the clipped head but large English
   blocks follow.

~1.0% of *cleaned* docs are English-heavy by a stopword heuristic; this drops to
~0.38% in the partial LID-kept output, so doc-level LID removes some but not all. A
whole-document winner cannot see a clean-Somali-header / English-body document.

### G. HTML entities — effectively solved

Only 0.02% of cleaned docs still contain encoded entities. The decode-once step
(`clean/entities.rs`) works; no action needed.

### H. Short-document tail

~5.8% of kept docs are `< 25` words and ~17% are `< 40`. This is **by design**: the
document floor is 25 words (validated in `reports/min_word_threshold_benchmark.md`)
and sentence-class sources (mt560/opus) legitimately sit below 25. Flagged here only
so the volume is a conscious choice, not an accident — see §4.H.

### I. Cross-document boilerplate

Not yet measurable (near-dedup pending). Within-document line repetition is already
low (≈1 in 15k docs has a line repeated ≥4×), so the risk is *cross*-document
templated boilerplate, which MinHash/LSH is designed to catch. Re-audit after the
stage completes.

---

## 4. Proposed v0.2 strategy

Ordered by value-to-risk. Every step keeps the pipeline's existing contract: stream
where possible, route removals to a sidecar with a reason, never silently delete, and
write before/after counts to the stage report.

### Priority 1 — Widen mojibake repair to split indicators (finding A)

- **What:** before the round-trip in `clean/mojibake.rs`, add a narrow pre-pass that
  rejoins whitespace-split indicator pairs (`Ã ¤` → `Ã¤`, `â€ ™` → `â€™`) *only* when a
  known continuation byte follows a lone indicator lead across a single space. Then the
  existing improve-only round-trip runs unchanged.
- **Why safe:** the improve-only guard (accept only if indicator count drops and no new
  U+FFFD) still governs the result, so a wrong rejoin is discarded. Keep the change
  behind the same acceptance test.
- **Golden set:** extend `tests/mojibake_golden.rs` with real split-indicator lines
  sampled from the survivors above. Do not synthesize them.
- **Expected effect:** removes most of the 0.11% survivor tail.

### Priority 2 — Strip stray U+FFFD from otherwise-kept docs (finding B)

- **What:** in `clean/strip.rs` (invisible-char stripping), also drop isolated U+FFFD.
  Order matters: this runs **after** mojibake repair and **after** the U+FFFD *ratio*
  gate has had its say, so heavily-corrupted docs are still rejected on ratio, but a
  surviving lone `�` in a kept doc is removed rather than shipped.
- **Why safe:** U+FFFD is never legitimate content; removing it cannot corrupt real text.
- **Knob:** none needed; keep `ufffd_reject_ratio` as the reject gate.

### Priority 3 — Split the HTML-remnant policy: strip scaffolding, keep prose (finding C)

The blanket flag-don't-strip rule is too coarse. Replace it with a two-tier policy in
a new `clean` step run **before** the length gate:

- **Tier 1 — reject** documents containing executable/structural scaffolding that marks
  the record as not-prose: `<?php`, `<script>…</script>` (and its contents),
  `<style>…</style>`, `<mutation>`, `<char>`. These are extraction failures, not text.
- **Tier 2 — strip-then-keep** benign inline tags (`<a>`, `<br>`, `<p>`, `<b>`, `<i>`,
  `<span>`, `<li>`, `<div>`) by removing the tag but preserving inner text, then re-run
  whitespace normalization.
- **Keep the flag** (`HtmlRemnant`) on any doc that still matches the tag regex after
  stripping, so residue remains auditable.
- **Why now and not in v0.1:** v0.1 deferred stripping to avoid destroying text before
  we had data. We now have data: the junk tier is a distinct, small, safely-identifiable
  population. Gate the reject tier conservatively (exact tag tokens, not a broad regex)
  and route rejects to the sidecar for spot-checking.
- **Risk:** medium. Validate on the `HtmlRemnant`-flagged sidecar population before and
  after; require that Tier-1 rejects are visually confirmed junk on a sample.

### Priority 4 — URL/email handling (findings D, E) — decide policy first

This is a policy decision, not a bug fix; pick one and record it in the changelog:

- **Option A (recommended for pretraining): mask, don't drop.** Replace URLs with a
  `⟨url⟩` sentinel and emails with `⟨email⟩` in `clean` (new step, after entity/mojibake,
  before whitespace). Preserves sentence structure and surrounding Somali text while
  removing low-value tokens and email PII.
- **Option B: strip the token entirely.** Cleaner text, but can leave dangling
  punctuation and fragment sentences.
- **Option C: keep as-is, add a `HasContact`/`HasUrl` review flag only.** Lowest risk,
  defers the decision, gives near-dedup and any future quality filter a signal.

Recommendation: **Option A** for URLs and emails, because the corpus target is language
modeling and raw URLs/emails are both noise and PII. Make the sentinels configurable and
count replacements in the stage report. Whichever option is chosen, add
`mask_urls`/`mask_emails` booleans to `[clean]` so the behavior is one versioned knob.

### Priority 5 — Segment-level LID for code-switch (finding F)

- **What:** augment the document-level gate in `clean/lid` with a **line/segment-level**
  pass: run LID per paragraph, and either (a) drop non-Somali segments while keeping the
  Somali ones, or (b) reject the whole doc when the Somali *character* fraction across
  segments falls below a threshold (e.g. < 0.6). Option (b) is simpler and lower-risk;
  option (a) risks fragmenting documents and should be prototyped before adoption.
- **Also:** stop clipping to the first 2000 bytes for the *decision* on long documents,
  or sample from multiple offsets, so a clean Somali header cannot mask an English body.
- **Why:** doc-level winner-take-all is structurally blind to mixed-language documents;
  the ~0.38% that survive into LID-kept output are exactly these.
- **Risk:** medium; benchmark against FLORES-200 confusables (already the LID eval basis
  in `CLEANING_PLAN.md §3`) and require no regression on monolingual Somali recall.

### Priority 6 — Confirm short-doc & mostly-numbers policy (finding H)

No code change proposed; a decision to record:

- Keep the 25-word document floor (it is benchmark-backed).
- Consider promoting `MostlyNumbers` + `HighSymbolRatio` + a new `HasContact` flag from
  *review* to *reject* **only** once the deferred seed-based quality filter
  (`CLEANING_PLAN.md §5`) exists to calibrate the thresholds. Until then, flag-only is
  correct; document the retained tail in release notes as already planned.

### Priority 7 — Re-audit near-dedup output (finding I)

Once near-dedup finishes, re-run the cross-document boilerplate probe and confirm the
templated-footer population (contact lines, "share this", navigation) is collapsed.
If residual near-identical boilerplate remains above ~1%, revisit the τ=0.80 threshold
or add a boilerplate-line filter upstream.

---

## 5. Proposed config additions

New keys under `[clean]` (all default to v0.1 behavior so the change is opt-in and the
config remains the single source of truth):

```toml
[clean]
# v0.2 additions
strip_benign_html   = true        # Tier-2: remove inline tags, keep inner text
reject_script_html  = true        # Tier-1: reject <?php/<script>/<style>/<mutation>
strip_stray_ufffd   = true        # drop lone U+FFFD from kept docs
mask_urls           = true        # URLs → ⟨url⟩ sentinel
mask_emails         = true        # emails → ⟨email⟩ sentinel

[lid]
# v0.2 additions
segment_level        = true       # per-paragraph LID in addition to doc-level
min_somali_char_frac = 0.6        # reject doc if Somali segment fraction below this
```

---

## 6. Validation plan

Each v0.2 change ships with before/after evidence, mirroring the existing benchmark
reports:

1. **Golden tests** for mojibake split-indicators and HTML tiering (real sampled lines,
   committed under `crates/corpus-pipeline/tests/`).
2. **Sidecar diffing:** every new removal writes to the reject sidecar with a distinct
   reason; a reviewer spot-checks 50 sampled rejects per new reason and confirms none
   are good Somali prose.
3. **Rate re-measurement:** re-run the §1 probes on v0.2 output and record the new
   per-document defect rates in a `reports/03_cleaning_audit.md` companion, targeting:
   mojibake < 0.02%, stray U+FFFD ≈ 0%, script/PHP remnants ≈ 0%, benign-tag residue
   < 0.1%, code-switch (English-heavy) < 0.2%.
4. **No-regression checks:** monolingual-Somali LID recall on FLORES-200 must not drop;
   total kept-document count must not fall more than the sum of the intended reject tiers.

---

## 7. Open questions

1. URL/email policy: mask vs. strip vs. flag (§4.4) — recommended **mask**, needs sign-off.
2. HTML Tier-1 reject list: is the token set (`<?php`, `<script>`, `<style>`,
   `<mutation>`, `<char>`) complete, or should it be data-driven from the full
   `HtmlRemnant` sidecar rather than the sample?
3. Segment-level LID: drop-segment vs. reject-document — prototype both, measure text
   fragmentation before committing.
4. Whether to promote `MostlyNumbers`/`HighSymbolRatio` to reject before the seed-based
   quality filter lands, or hold until it can calibrate thresholds.
