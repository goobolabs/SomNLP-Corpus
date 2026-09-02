# Optimizing Tokenization Efficiency for the Somali Language Using BPE

**SomNLP-Corpus Tokenizer Research Note**

---

## Abstract

This document describes the design, training procedure, and measured efficiency of corpus-native Byte Pair Encoding tokenizers for the Somali language (`so`), trained on the SomNLP-Corpus thirteen-source Track A release (7,352,961 documents, 665,985,672 words, 2026-09-02). Standard English-centric and general multilingual subword models impose substantial *token-per-word inflation* on Somali text because their merge inventories were optimized for typologically and orthographically distant languages.

Two models are documented. **v1** (32,000 types, whitespace pre-tokenization) was the initial release; it is retained unchanged for reproducibility but is unsuitable for generative use, because its decoder was configured for an end-of-word suffix the trainer never emitted and therefore fails to reconstruct whitespace on **every** document tested, and because out-of-alphabet characters collapse to an unknown token. **v2** (48,000 types, ByteLevel pre-tokenization and decoding) is the shipped model. It reconstructs its input exactly, cannot emit an unknown token because all 256 bytes seed the alphabet, and is nonetheless *more* efficient than v1: mean 1.3468 tokens per word against 1.3885 on a held-out split of 59,029 documents, versus 2.6336 for BERT-base and 1.8158 for XLM-RoBERTa-base on identical text.

The vocabulary size was selected from a measured four-point sweep rather than assumed, and both correctness properties are enforced by a property test suite rather than asserted in prose. The result is intended for pretraining and downstream Garaad Gacmeed systems where context-window utilization and inference cost are first-order engineering constraints.

---

## 1. Introduction and Motivation

Somali (`Af-Soomaali`) is a Cushitic language written predominantly in the Latin script, with rich inflectional and derivational morphology and frequent multi-word religious, legal, and journalistic collocations in contemporary web text. Despite growing public corpora and parallel resources, Somali remains under-represented in the subword inventories of widely deployed pretrained language models. When such models are applied to Somali downstream tasks—machine translation, summarization, question answering, retrieval-augmented dialogue, and domain-specific assistants—the mismatch between tokenizer statistics and Somali word formation produces systematic inefficiency.

For **Garaad Gacmeed** (Artificial Intelligence) applications in Somali, this inefficiency is not a cosmetic concern. Modern transformer architectures bound usable context by a fixed token budget. If each Somali word consumes two to three times as many tokens under an English tokenizer as under a native one, then effective context length, batch throughput, and API billing all degrade proportionally. A 4,096-token window that might accommodate roughly 3,000 Somali words under an optimal tokenizer may hold fewer than 1,500 words under a poorly matched one, directly limiting document-level reasoning, long-context retrieval, and multi-turn conversational memory.

Subword fragmentation also interacts with model quality. Rare merges force characters or short fragments into the unknown-token bucket, increase sequence length variance across domains, and amplify exposure bias during autoregressive decoding. For low-resource language technology programs, tokenizer design is therefore a co-equal decision with architecture and data curation: it defines the atomic units of prediction.

The SomNLP-Corpus project addresses data quality through a six-stage Rust pipeline (merge, clean, language identification, deep clean, near deduplication, and final release). The present work closes the loop by deriving a tokenizer from the same release artifact used for language-model pretraining, ensuring consistency between training text normalization and subword statistics.

---

## 2. Background

### 2.1 Subword tokenization

Subword tokenization represents text as a sequence of units drawn from a finite vocabulary \(\mathcal{V}\). Byte Pair Encoding, introduced by Sennrich et al. (2016) for neural machine translation, constructs \(\mathcal{V}\) iteratively from corpus statistics. Beginning from an inventory of symbols—typically bytes or Unicode characters—the algorithm repeatedly merges the most frequent adjacent symbol pair until a target vocabulary size \(|\mathcal{V}| = V\) is reached.

Formally, let \(f(a, b)\) denote the frequency of adjacent pair \((a, b)\) in the current corpus segmentation. At iteration \(t\):

\[
(a^*, b^*) = \arg\max_{(a,b)} f(a,b), \quad \text{subject to } f(a,b) \geq \tau
\]

where \(\tau\) is a minimum frequency threshold. The merge operator replaces every occurrence of the concatenation \(ab\) with a new symbol \(m_t\), updates frequencies, and continues until \(|V|\) special and merged tokens are present. Decoding inverts merges by longest-match greedy segmentation against the merge rank table.

### 2.2 Why Somali stresses foreign tokenizers

Somali orthography uses the Latin alphabet with additional characters (e.g., `x`, `q`, `dh`, `kh`) and apostrophe-mediated clitics (`'`, as in *reer binu Israa 'iil*). Web corpora contain loanwords, religious Arabic names transliterated into Somali spelling conventions, numerals, punctuation-heavy parenthetical structures, and masked URL/email sentinels (`⟨url⟩`, `⟨email⟩`) introduced during corpus hygiene. English BPE models prioritize merges attested in English morphology and punctuation patterns; multilingual models balance dozens of scripts and languages, diluting Somali-specific merges. The consequence is elevated token counts per whitespace-delimited word—a practical proxy for morphological alignment—even when character error rates appear acceptable.

---

## 3. Corpus and Preprocessing Methodology

### 3.1 Source data

Training text is extracted exclusively from the release corpus:

| Attribute | Value |
|-----------|------:|
| Path | `data/final/final_so.jsonl` |
| Documents | 7,352,961 |
| Words | 665,985,672 |
| On-disk size | 7.17 GiB |
| Schema | `CorpusRecord` v1 (`text`, `id`, `provenance.source`) |
| Build date | 2026-09-02 (thirteen-source Track A) |

Upstream processing already applied HTML entity decoding, mojibake repair, Unicode NFC normalization, invisible-character stripping, whitespace normalization, language identification (`lingua`), boilerplate removal, segment-level Somali verification, intra-document paragraph deduplication, and MinHash near deduplication. Rejected sidecars (`*.rejected.jsonl`, `*.dropped.jsonl`) are excluded.

**Composition.** Document share and word share diverge sharply, and word share is what governs merge learning: BPE counts word frequencies, so a source's influence on the merge table is proportional to the words it contributes, not the records. NLLB supplies 55.9% of documents but only 9.3% of words, because its records are short parallel sentences averaging 15 words; Glot500 shows the same pattern at 19.5% of documents for 10.1% of words. Conversely the four web-crawl sources supply 21.8% of documents and 79.1% of words. The resulting merge table is, in effect, a web-crawl tokenizer with a thin editorial and religious tail.

| Source | Documents | Doc share | Words | Word share | Words/doc |
|--------|----------:|----------:|------:|-----------:|----------:|
| `mc4` | 590,121 | 8.1% | 218,605,468 | 33.1% | 370 |
| `hplt` | 572,208 | 7.8% | 190,052,829 | 28.8% | 332 |
| `glot` | 1,421,843 | 19.5% | 66,501,139 | 10.1% | 47 |
| `madlad` | 132,317 | 1.8% | 64,487,875 | 9.8% | 487 |
| `nllb` | 4,075,196 | 55.9% | 61,571,557 | 9.3% | 15 |
| `cc100` | 296,770 | 4.1% | 49,448,818 | 7.5% | 167 |
| `somali-web-corpus` | 120,854 | 1.7% | 4,855,405 | 0.7% | 40 |
| `xlsum` | 5,603 | 0.1% | 2,004,843 | 0.3% | 358 |
| `wikipedia` | 5,426 | 0.1% | 1,186,834 | 0.2% | 219 |
| `mt560` | 48,811 | 0.7% | 1,153,705 | 0.2% | 24 |
| `opus` | 12,037 | 0.2% | 378,964 | 0.1% | 31 |
| `quran` | 7,018 | 0.1% | 158,794 | 0.0% | 23 |
| `quran-tanzil` | 5,722 | 0.1% | 105,227 | 0.0% | 18 |

*(Training split only; the evaluation holdout mirrors these shares to within 0.3 percentage points.)*

### 3.2 Corpus preparation (`prepare_corpus.py`)

The preparation script streams JSONL records once and writes two artifacts. A lightweight post-pass applies tokenizer-oriented normalization without altering the semantic content of the release corpus:

1. **NFC normalization** — stabilizes composed characters for consistent merge counts.
2. **Paragraph blank-line collapse** — reduces runs of three or more newlines to a double newline.
3. **Empty-document filtering** — skips records whose `text` field is blank after stripping.

Corpus-level sentinels for masked URLs and emails are preserved so the tokenizer can learn dedicated subword representations for them.

**`somali_raw_corpus.txt`** carries the training text. Paragraph structure is retained, so a multi-paragraph document occupies several lines: 1,293,324 of 7,293,926 training documents (17.7%) contain at least one internal newline, essentially all of them from the document-class web sources. This file is therefore *not* one document per line. Earlier versions of this document claimed otherwise, which also meant the v1 benchmark's "per-document" ratios were in fact computed per paragraph for web-sourced text.

**`eval_holdout.jsonl`** carries the evaluation split as one JSON record per line (`id`, `source`, `text`), which keeps documents containing newlines addressable as single units and preserves provenance for per-source reporting. Assignment is deterministic: a document is held out when `blake2b-8(id) mod 1000 < 8`. The split needs no stored index, is reproducible from the record id alone, and is stable across corpus rebuilds for documents whose id is unchanged.

| Split | Documents | Words |
|-------|----------:|------:|
| Train | 7,293,926 | 660,511,458 |
| Evaluation holdout | 59,029 | 5,474,214 |

The two splits sum to 7,352,955 rather than the corpus's 7,352,961: six records are empty
after stripping and NFC normalization and are skipped.

### 3.3 Training configuration (`train.py`)

v1 and v2 differ in the pre-tokenization and decoding path. That single axis determines whether decoding is reversible and whether an unknown token can occur at all.

| Hyperparameter | v1 (shipped 2026-07-07) | v2 | Rationale for v2 |
|----------------|------------------------|----|------------------|
| Algorithm | BPE | BPE | Unchanged |
| Target vocabulary \(V\) | 32,000 | swept: 16,384 / 32,000 / 48,000 / 65,536 | Chosen empirically (§6.3) |
| Minimum merge frequency | 2 | 2 | Suppress hapax-driven noise |
| Normalizer | NFC | NFC | Matches corpus convention |
| Pre-tokenizer | `Whitespace` | `ByteLevel(add_prefix_space=False)` | Reversible; `add_prefix_space=False` keeps round-trip byte-exact |
| Decoder | `BPEDecoder` | `ByteLevel` | v1's decoder expected an `</w>` suffix the trainer never emitted, so decoding concatenated all tokens |
| Post-processor | none | `ByteLevel(trim_offsets=True)` | Correct character offsets |
| Initial alphabet | corpus characters | all 256 bytes | Makes `<unk>` unreachable |
| Special tokens | `<unk>`, `[CLS]`, `[SEP]`, `<pad>`, `[MASK]` | `<|endoftext|>`, `<|pad|>`, `<|im_start|>`, `<|im_end|>`, 12 reserved | Decoder-LM control set; reserved slots avoid a later embedding resize |

Training uses Hugging Face `tokenizers` with a streaming iterator over the plain-text file, so the full corpus is never resident in memory.

---

## 4. BPE Training Procedure

The implementation follows the standard Hugging Face training loop:

1. Seed the vocabulary with all 256 byte symbols.
2. Reserve index positions for special tokens.
3. Count adjacent pairs on ByteLevel-pretokenized words.
4. Merge the highest-frequency pair; append merged symbol to \(\mathcal{V}\).
5. Repeat until \(|\mathcal{V}|\) reaches the target size.
6. Serialize merge ranks, vocabulary, and normalization metadata to JSON.

Because the merge rule is greedy and deterministic, the first \(k\) merges do not depend on the target vocabulary size; a larger run's merge list is a superset of a smaller one's.

Measured cost on eight CPU cores: corpus preparation over 7,352,961 records takes 174 s, and a single full-corpus training run completes in minutes rather than hours. A `--limit-lines` flag supports smoke testing on subsets before production runs.

### 4.1 Somali-specific tokenization mechanics

Several phenomena influence merge structure:

- **Agglutination and function words** — high-frequency particles (`waa`, `oo`, `ka`, `ku`, `ay`) and verbal morphology produce recurrent suffix chains that BPE merges into stable multi-character tokens when attested at scale.
- **Apostrophe boundaries** — clitic apostrophes are intra-word characters under both pre-tokenizers, allowing merges to span or respect clitic boundaries according to corpus frequency rather than hand-crafted rules.
- **Religious and legal register** — MT560 and the Quran sources contribute repeated formulaic phrases, though at 0.2% of corpus words their influence on the merge table is slight.
- **Sentinel tokens** — masked `⟨url⟩` and `⟨email⟩` spans appear in roughly 16% of documents. They do *not* automatically become single tokens: v1 segments `⟨url⟩` into three units (`⟨`, `url`, `⟩`), because the delimiters are non-ASCII and comparatively rare. Whether v2 merges them is a function of vocabulary size and is reported in §6.

---

## 5. Validation Protocol

Validation has three layers.

**Unit and property tests (`tests/`).** A pytest suite runs against a 2,000-token tokenizer trained on a committed 210-document fixture spanning all thirteen sources, so the whole suite completes in under a second with no network access and no dependency on the 7 GB corpus. It asserts the properties that motivated v2:

- `decode(encode(x))` reproduces `NFC(x)` for every fixture document and for an adversarial set covering tabs, repeated spaces, paragraph breaks, emoji, CJK, runic script, the `⟨url⟩`/`⟨email⟩` sentinels, apostrophe clitics, and empty input;
- the vocabulary contains no `<unk>` token and the 256-symbol byte alphabet is complete, so unknown-token loss is unreachable rather than merely unobserved;
- every special token encodes to exactly one id and survives `decode(..., skip_special_tokens=False)`.

Round-trip equality is asserted against the NFC-normalized input, not the raw input. The normalizer is part of the tokenizer by design and the release corpus is already NFC, so this is the meaningful invariant; a decomposed sequence such as `a`+U+0301 is deliberately returned composed.

**Qualitative inspection (`benchmark.py`).** Fixed Somali sentences — agglutinative constructions, parenthetical religious text, and sentinel-bearing lines — are encoded to surface tokens with per-sentence ratios and a round-trip verdict.

**Quantitative benchmark (`benchmark.py`).** Every tokenizer is scored on the same held-out documents in a single streaming pass. For each document \(d\), with \(W(d)\) the whitespace-delimited word count and \(T(d)\) the token count, the primary metric is

\[
R(d) = \frac{T(d)}{W(d)}
\]

reported as mean, median, and 95th percentile, alongside bytes-per-token (a vocabulary-size-independent compression measure), round-trip fidelity rate, unknown-token rate, and a per-source breakdown. Optional `bert-base-uncased` and `xlm-roberta-base` baselines are scored on the identical documents and are skipped cleanly when unavailable offline.

Only the per-document ratios are retained during the pass, never the document strings. The previous implementation materialized the entire sample as a Python list, which at full corpus size would have required an estimated 10–14 GB of resident memory; the streaming form uses tens of megabytes.

---

## 6. Results

### 6.1 Protocol

All figures below come from a single scoring pass over the held-out evaluation split:
**59,029 documents / 5,474,214 words**, excluded from v2 training. v1 was trained before
this split existed and its training corpus overlapped it; that bias runs *in v1's favour*, so
a v2 win here is conservative.

Because the split is a hash of the record id (§3.2), it is stable across corpus rebuilds:
every id held out of the thirteen-source corpus was also held out of the eleven-source
corpus v2 trained on, so growing the corpus introduced no leakage. It did introduce
*unseen* text — the holdout now contains Glot500 and Somali Web Corpus documents from two
sources v2's merge table never saw. Those are reported in §6.4 and are the honest test of
whether the shipped vocabulary generalizes.

Two changes make these numbers non-comparable with the figures published for v1 on
2026-07-07. First, the unit is now the document: the earlier benchmark sampled *lines* of
`somali_raw_corpus.txt`, and because 18% of documents contain internal newlines (§3.2), those
"documents" were largely paragraphs. Second, the corpus itself grew from six sources to
thirteen. The v1 column below is a fresh measurement under the current protocol, not the
previously published 1.53.

### 6.2 v1 versus v2

| Tokenizer | Mean \(R(d)\) | Median | P95 | Bytes/token | Docs with `<unk>` | Round-trip fidelity |
|-----------|--------------:|-------:|----:|------------:|------------------:|--------------------:|
| **v2 ByteLevel 48k (shipped)** | 1.3468 | 1.2766 | 1.8462 | 4.820 | 0 | **1.000** |
| v2 ByteLevel 65,536 | 1.3142 | 1.2500 | 1.7917 | 4.942 | 0 | **1.000** |
| v1 Whitespace 32k | 1.3885 | 1.3171 | 1.9091 | 4.738 | 4 | **0.000** |
| XLM-RoBERTa-base | 1.8158 | 1.7500 | 2.4167 | 3.671 | 0 | n/a |
| BERT-base-uncased | 2.6336 | 2.6170 | 3.2292 | 2.526 | 0 | n/a |

The shipped v2 model is better than v1 on every ratio metric — mean 1.3468 against
1.3885, a 3.0% reduction — while also being reversible. This is the result that
matters: the correctness fix did not cost compression, it improved it, because the byte
alphabet frees the merge table from spending capacity on rare characters.

**Round-trip fidelity is the decisive column.** v1 fails on 59,029 of 59,029
documents — every single one. Its `BPEDecoder` was configured to strip an `</w>` suffix
that the trainer never emitted, so decoding concatenates tokens with no separator:
`"Soomaaliya waa dal"` decodes as `"Soomaaliyawaadal"`. For a classification or retrieval
encoder that is invisible; for a generative model it is disqualifying. v1 also emitted
`<unk>` on 4 held-out documents. v2 scores 1.000 fidelity and cannot
produce an unknown token at all.

Against external baselines, v2 fragments Somali 1.96× less than BERT-base and
1.35× less than XLM-RoBERTa-base on identical text.

**Corpus-level total, measured not extrapolated.** `tokenize_corpus.py` encodes every
document in `final_so.jsonl` — no sampling, no holdout ratio applied to a word count. The
shipped v2 model yields **911,824,557 tokens** over 665,985,672 words, a corpus-wide ratio
of **1.3691** at 4.836 bytes/token. The full pass also fixes the document-length
distribution used for context-window planning: 80.97% of documents encode to fewer than 128
tokens (Glot500 and NLLB are sentence-level), and only 1.84% exceed 1,024.

| Tokenizer | Corpus tokens | Tokens/word | Bytes/token |
|-----------|--------------:|------------:|------------:|
| v2 ByteLevel 65,536 | 889,663,181 | 1.3359 | 4.956 |
| **v2 ByteLevel 48k (shipped)** | **911,824,557** | **1.3691** | 4.836 |
| v1 Whitespace 32k | 927,595,947 | 1.3928 | 4.754 |
| v2 ByteLevel 32,000 | 947,053,346 | 1.4220 | 4.656 |
| v2 ByteLevel 16,384 | 1,024,005,031 | 1.5376 | 4.306 |

Round-trip was verified during the same pass on every 500th document (14,722 checks per
tokenizer): v2 fails none, v1 fails all of them.

### 6.3 Vocabulary size

Measured, not projected. Because BPE merges are greedy, the four candidates differ only in
where the shared merge list is cut (§4), so this table isolates vocabulary size exactly.

| Vocabulary | Mean \(R(d)\) | Median | P95 | Bytes/token | Holdout tokens |
|-----------:|--------------:|-------:|----:|------------:|---------------:|
| 16,384 | 1.5132 | 1.4286 | 2.1538 | 4.291 | 8.44M |
| 32,000 | 1.3989 | 1.3243 | 2.0000 | 4.639 | 7.81M |
| 48,000 | 1.3468 | 1.2766 | 1.8462 | 4.820 | 7.51M |
| 65,536 | 1.3142 | 1.2500 | 1.7917 | 4.942 | 7.33M |

Returns diminish steadily. Moving 16,384 -> 32,000 buys 7.6% off the mean; 32,000 -> 48,000
buys 3.7%; 48,000 -> 65,536 buys only 2.4% for a further 36% of embedding parameters. **48,000 is
shipped** as the point where v2 clears v1 on every metric at a defensible embedding cost;
65,536 remains in `sweep/` for deployments where context length dominates parameter budget.

### 6.4 Per-source efficiency

Mean tokens/word by source (holdout):

| Source | v1 | v2-48k | XLM-R |
|--------|---:|-------:|------:|
| `mc4` | 1.4876 | 1.4683 | 1.8741 |
| `hplt` | 1.3503 | 1.3419 | 1.7576 |
| `glot` | 1.3844 | 1.3370 | 1.8071 |
| `madlad` | 1.3482 | 1.3324 | 1.7661 |
| `nllb` | 1.3711 | 1.3211 | 1.8012 |
| `cc100` | 1.2868 | 1.2413 | 1.7083 |
| `somali-web-corpus` | 1.2955 | 1.2472 | 1.6902 |
| `xlsum` | 1.3344 | 1.2841 | 1.7244 |
| `wikipedia` | 1.4883 | 1.4836 | 1.8331 |
| `mt560` | 1.2936 | 1.2292 | 1.8067 |
| `opus` | 1.4081 | 1.3521 | 1.8012 |
| `quran` | 1.5785 | 1.5267 | 2.0063 |
| `quran-tanzil` | 1.6171 | 1.5460 | 2.0635 |

The religious sources are the hardest for every tokenizer, and the gap does not close with
a native vocabulary: `quran-tanzil` costs v2 1.5460 tokens/word against 1.2413 for
`cc100`. This is a direct consequence of the composition reported in §3.1 — those sources
contribute 0.04% of training words, so few merges are shaped by their orthography.

**The two unseen sources generalize.** `glot` (1.3370) and `somali-web-corpus` (1.2472) are
absent from v2's training corpus entirely, yet both tokenize *better* than the holdout mean
of 1.3468 and better than v1 does on the same documents (1.3844 and 1.2955). A vocabulary
learned on eleven sources transfers to the thirteen-source corpus without measurable loss,
which is why v2 was not retrained when the corpus grew.

### 6.5 Limitations

1. **Script coverage** — the corpus is Latin-script Somali; Arabic-script text is out of distribution. It will encode without loss under v2, but inefficiently.
2. **Domain bias** — 79.1% of training words come from web crawl (§3.1). The merge table reflects web register far more than edited, spoken, or administrative Somali.
3. **Whitespace word definition** — \(R(d)\) uses whitespace splitting, which treats clitics and attached punctuation inconsistently across sources. Bytes/token is reported alongside as a definition-free compression measure.
4. **Baseline mismatch** — BERT and XLM-R use different pre-tokenizers and normalizers; the comparison isolates tokenizer efficiency, not end-task accuracy.
5. **v1's holdout overlap** — v1 saw an earlier version of these documents in training, so its numbers here are, if anything, flattering.
6. **Round-trip is exact with respect to NFC**, not to arbitrary byte sequences; the normalizer is part of the tokenizer and the corpus is already NFC.
7. **Vocabulary predates two sources** — v2's merge table was learned on the eleven-source corpus. Glot500 and Somali Web Corpus were added afterwards and contribute 10.8% of corpus words. §6.4 shows both tokenize better than the corpus mean, so the shipped model was kept; a thirteen-source retrain would move the headline by less than the sweep's smallest step.

---

## 7. Pipeline Usage

```bash
cd tokenizer
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt

pytest                             # unit + property tests, <1s, no network
python prepare_corpus.py --stats   # reads data/final/final_so.jsonl, writes train text + holdout
python train.py                    # trains one tokenizer at the default vocabulary size
python benchmark.py                # scores v1 and v2 on the holdout
python tokenize_corpus.py          # encodes every document; measured corpus token total
```

Vocabulary sweep and comparison:

```bash
python train.py --sweep 16384,32000,48000,65536
python benchmark.py --sweep-dir tokenizer/sweep
```

Paths passed on the command line resolve against the repository root, not `tokenizer/`.

Full-corpus pass across several tokenizers at once:

```bash
python tokenize_corpus.py --jobs 7 \
  --tokenizer "v1=tokenizer/somali-bpe-tokenizer.json" \
  --tokenizer "v2-48000=tokenizer/somali-bpe-v2.json"
```

`tokenize_corpus.py` shards the corpus by byte range across a process pool. Each worker
returns counters only — never texts — and percentiles come from a mergeable fixed-width
histogram, so peak memory is independent of corpus size. A thirteen-source pass over five
tokenizers takes about 65 minutes on eight cores.

Smoke-test the whole path on a subset before a production run:

```bash
python prepare_corpus.py --limit 10000 --stats
python train.py --limit-lines 10000
python benchmark.py --no-baseline --limit 1000
```

`train.py` refuses to run when `somali_raw_corpus.txt` does not derive from the current
`final_so.jsonl`, comparing a fingerprint (size, mtime, hashed head and tail) recorded by
`prepare_corpus.py`. This guard exists because the previous version checked only that the
training file existed, and would silently retrain on a months-old corpus. Override with
`--allow-stale` when that is genuinely intended.

---

## 8. Conclusion

Training BPE on the SomNLP-Corpus final release aligns subword statistics with attested Somali morphology and orthography, addressing a structural bottleneck for Garaad Gacmeed systems deployed on transformer architectures. By reducing token-per-word inflation relative to English and multilingual off-the-shelf tokenizers, a native model increases usable context, improves training data efficiency, and lowers inference cost at fixed linguistic content. The pipeline implemented in `tokenizer/` is modular, streaming, and reproducible, and is covered by a property test suite asserting the two invariants a decoder LM depends on: that decoding inverts encoding exactly, and that no input can fall back to an unknown token. v1 remains in the repository unchanged so its published figures stay reproducible; v2 supersedes it for any generative use.

---

## References

- Sennrich, R., Haddow, B., & Birch, A. (2016). *Neural Machine Translation of Rare Words with Subword Units.* ACL.
- Hugging Face. *Tokenizers Library Documentation.* https://huggingface.co/docs/tokenizers
- SomNLP-Corpus Project. *README.md*, *CLEANING_STRATEGY.md*, *DATA_PIPELINE.md.*
- Devlin, J., et al. (2019). *BERT: Pre-training of Deep Bidirectional Transformers for Language Understanding.* NAACL.
- Conneau, A., et al. (2020). *Unsupervised Cross-lingual Representation Learning at Scale.* ACL (XLM-R).

---

*Document version: 2.1 — v2 ByteLevel tokenizer on the thirteen-source corpus: held-out split of 59,029 documents plus a measured full-corpus pass (2026-09-02).*
