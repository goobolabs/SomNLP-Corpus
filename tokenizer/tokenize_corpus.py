#!/usr/bin/env python3
"""Tokenize the entire corpus and report measured token statistics.

benchmark.py scores the ~49k held-out documents; this script makes a full pass over
every document in data/final/final_so.jsonl so the corpus token count is measured
rather than extrapolated from the holdout ratio.

The file is split into byte ranges and processed by a pool of workers. Each worker
returns counters only - never texts - so peak memory is independent of corpus size.
Percentiles come from a fixed-width ratio histogram, which is mergeable across
workers and accurate to the bin width (0.005 tokens/word).
"""

from __future__ import annotations

import argparse
import json
import logging
import multiprocessing as mp
import os
import time
import unicodedata
from collections import defaultdict
from pathlib import Path

from tokenizers import Tokenizer

from common import (
    DEFAULT_CORPUS_JSONL,
    DEFAULT_TOKENIZER_V2,
    TOKENIZER_DIR,
    add_repo_path_arg,
    clean_document,
    corpus_fingerprint,
    count_words,
    fail,
    resolve_under_repo,
    setup_logging,
    write_json,
)

# Ratio histogram: 4000 bins of 0.005 covers 0 to 20 tokens/word.
RATIO_BIN_WIDTH = 0.005
RATIO_BINS = 4000

# Document length buckets, in tokens. Sized around the context lengths a decoder LM
# would be trained at, so the tail is visible without storing per-document lengths.
LENGTH_EDGES = (128, 256, 512, 1024, 2048, 4096, 8192)

BATCH_SIZE = 10_000

# Sample one document in ROUNDTRIP_STRIDE for the decode(encode(x)) == x assertion.
ROUNDTRIP_STRIDE = 500


def length_bucket(n_tokens: int) -> str:
    lower = 0
    for edge in LENGTH_EDGES:
        if n_tokens < edge:
            return f"{lower}-{edge}"
        lower = edge
    return f"{LENGTH_EDGES[-1]}+"


class Tally:
    """Mergeable counters for one tokenizer over one shard of the corpus."""

    def __init__(self, label: str) -> None:
        self.label = label
        self.documents = 0
        self.words = 0
        self.characters = 0
        self.bytes = 0
        self.tokens = 0
        self.roundtrip_checked = 0
        self.roundtrip_failures = 0
        self.ratio_hist = [0] * RATIO_BINS
        self.length_hist: dict[str, int] = defaultdict(int)
        # A plain dict, not a defaultdict with a lambda: shard tallies are pickled
        # back from the worker pool and a lambda factory is not picklable.
        self.by_source: dict[str, dict[str, int]] = {}

    def _source(self, source: str) -> dict[str, int]:
        counts = self.by_source.get(source)
        if counts is None:
            counts = dict.fromkeys(("documents", "words", "characters", "bytes", "tokens"), 0)
            self.by_source[source] = counts
        return counts

    def add(self, source: str, text: str, n_tokens: int) -> None:
        words = count_words(text)
        nbytes = len(text.encode("utf-8"))
        self.documents += 1
        self.words += words
        self.characters += len(text)
        self.bytes += nbytes
        self.tokens += n_tokens
        self.length_hist[length_bucket(n_tokens)] += 1
        src = self._source(source)
        src["documents"] += 1
        src["words"] += words
        src["characters"] += len(text)
        src["bytes"] += nbytes
        src["tokens"] += n_tokens
        if words:
            index = min(int((n_tokens / words) / RATIO_BIN_WIDTH), RATIO_BINS - 1)
            self.ratio_hist[index] += 1

    def merge(self, other: Tally) -> None:
        self.documents += other.documents
        self.words += other.words
        self.characters += other.characters
        self.bytes += other.bytes
        self.tokens += other.tokens
        self.roundtrip_checked += other.roundtrip_checked
        self.roundtrip_failures += other.roundtrip_failures
        for i, count in enumerate(other.ratio_hist):
            self.ratio_hist[i] += count
        for bucket, count in other.length_hist.items():
            self.length_hist[bucket] += count
        for source, counts in other.by_source.items():
            target = self._source(source)
            for key, value in counts.items():
                target[key] += value

    def percentile(self, q: float) -> float:
        total = sum(self.ratio_hist)
        if not total:
            return 0.0
        target = q * total
        seen = 0
        for index, count in enumerate(self.ratio_hist):
            seen += count
            if seen >= target:
                return round((index + 0.5) * RATIO_BIN_WIDTH, 4)
        return round(RATIO_BINS * RATIO_BIN_WIDTH, 4)

    def mean_ratio(self) -> float:
        total = sum(self.ratio_hist)
        if not total:
            return 0.0
        weighted = sum(c * (i + 0.5) * RATIO_BIN_WIDTH for i, c in enumerate(self.ratio_hist) if c)
        return round(weighted / total, 4)

    def summary(self) -> dict[str, object]:
        by_source = {}
        for source, counts in sorted(
            self.by_source.items(), key=lambda kv: kv[1]["tokens"], reverse=True
        ):
            by_source[source] = {
                **counts,
                "token_share": round(counts["tokens"] / self.tokens, 6) if self.tokens else 0.0,
                "tokens_per_word": round(counts["tokens"] / counts["words"], 4)
                if counts["words"]
                else 0.0,
                "bytes_per_token": round(counts["bytes"] / counts["tokens"], 4)
                if counts["tokens"]
                else 0.0,
            }
        return {
            "label": self.label,
            "documents": self.documents,
            "words": self.words,
            "characters": self.characters,
            "bytes": self.bytes,
            "total_tokens": self.tokens,
            "corpus_tokens_per_word": round(self.tokens / self.words, 4) if self.words else 0.0,
            "mean_tokens_per_word": self.mean_ratio(),
            "median_tokens_per_word": self.percentile(0.50),
            "p95_tokens_per_word": self.percentile(0.95),
            "p99_tokens_per_word": self.percentile(0.99),
            "bytes_per_token": round(self.bytes / self.tokens, 4) if self.tokens else 0.0,
            "characters_per_token": round(self.characters / self.tokens, 4)
            if self.tokens
            else 0.0,
            "roundtrip_checked": self.roundtrip_checked,
            "roundtrip_failures": self.roundtrip_failures,
            "roundtrip_fidelity": round(
                1.0 - self.roundtrip_failures / self.roundtrip_checked, 6
            )
            if self.roundtrip_checked
            else None,
            "document_length_histogram": {
                bucket: self.length_hist.get(bucket, 0) for bucket in _length_bucket_order()
            },
            "by_source": by_source,
        }


def _length_bucket_order() -> list[str]:
    order = []
    lower = 0
    for edge in LENGTH_EDGES:
        order.append(f"{lower}-{edge}")
        lower = edge
    order.append(f"{LENGTH_EDGES[-1]}+")
    return order


def iter_shard(path: Path, start: int, end: int):
    """Yield (source, cleaned_text) for whole lines whose start offset is in [start, end)."""
    with path.open("rb") as handle:
        handle.seek(start)
        if start:
            handle.readline()  # the line straddling `start` belongs to the previous shard
        while handle.tell() < end:
            raw = handle.readline()
            if not raw:
                return
            try:
                record = json.loads(raw)
            except json.JSONDecodeError:
                continue
            text = record.get("text")
            if not isinstance(text, str):
                continue
            cleaned = clean_document(text)
            if not cleaned:
                continue
            provenance = record.get("provenance")
            source = "unknown"
            if isinstance(provenance, dict) and isinstance(provenance.get("source"), str):
                source = provenance["source"]
            yield source, cleaned


def run_shard(job: tuple[Path, int, int, list[tuple[str, str]], bool]) -> list[Tally]:
    path, start, end, specs, check_roundtrip = job
    # Each worker already owns a core; let the Rust tokenizer stay single-threaded so
    # the pool does not oversubscribe.
    os.environ["TOKENIZERS_PARALLELISM"] = "false"
    tokenizers = [(label, Tokenizer.from_file(file)) for label, file in specs]
    tallies = [Tally(label) for label, _ in specs]

    sources: list[str] = []
    texts: list[str] = []
    seen = 0

    def flush() -> None:
        nonlocal seen
        if not texts:
            return
        for (_label, tok), tally in zip(tokenizers, tallies, strict=True):
            encodings = tok.encode_batch_fast(texts)
            for source, text, encoding in zip(sources, texts, encodings, strict=True):
                tally.add(source, text, len(encoding.ids))
            if check_roundtrip:
                for offset in range(0, len(texts), ROUNDTRIP_STRIDE):
                    text = texts[offset]
                    decoded = tok.decode(
                        encodings[offset].ids, skip_special_tokens=False
                    )
                    tally.roundtrip_checked += 1
                    if decoded != unicodedata.normalize("NFC", text):
                        tally.roundtrip_failures += 1
        seen += len(texts)
        sources.clear()
        texts.clear()

    for source, text in iter_shard(path, start, end):
        sources.append(source)
        texts.append(text)
        if len(texts) >= BATCH_SIZE:
            flush()
    flush()
    return tallies


def build_jobs(
    path: Path, jobs: int, specs: list[tuple[str, str]], check_roundtrip: bool
) -> list[tuple]:
    size = path.stat().st_size
    shards = jobs * 4  # oversubscribe so uneven shards do not stall the pool
    edges = [size * i // shards for i in range(shards + 1)]
    return [
        (path, edges[i], edges[i + 1], specs, check_roundtrip) for i in range(shards)
    ]


def parse_tokenizer_specs(values: list[str], repo_root: Path) -> list[tuple[str, str]]:
    specs = []
    for value in values:
        label, _, raw = value.partition("=")
        if not raw:
            label, raw = Path(value).stem, value
        resolved = resolve_under_repo(repo_root, Path(raw))
        if not resolved.is_file():
            fail(f"Tokenizer not found: {resolved}")
        specs.append((label, str(resolved)))
    return specs


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    add_repo_path_arg(parser)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS_JSONL)
    parser.add_argument(
        "--tokenizer",
        action="append",
        default=[],
        metavar="LABEL=PATH",
        help="Tokenizer to run; repeatable. Defaults to the shipped v2 artifact.",
    )
    parser.add_argument(
        "--output", type=Path, default=TOKENIZER_DIR / "corpus_token_stats.json"
    )
    parser.add_argument("--jobs", type=int, default=max(1, (os.cpu_count() or 2) - 1))
    parser.add_argument(
        "--no-roundtrip",
        action="store_true",
        help=f"Skip the decode(encode(x)) check sampled every {ROUNDTRIP_STRIDE} documents.",
    )
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()
    setup_logging(args.verbose)

    corpus = resolve_under_repo(args.repo_root, args.corpus)
    if not corpus.is_file():
        fail(f"Corpus not found: {corpus}")
    specs = parse_tokenizer_specs(
        args.tokenizer or [f"v2={DEFAULT_TOKENIZER_V2}"], args.repo_root
    )

    logging.info("Corpus: %s (%.2f GiB)", corpus, corpus.stat().st_size / 2**30)
    for label, path in specs:
        logging.info("Tokenizer %s: %s", label, path)

    started = time.time()
    jobs = build_jobs(corpus, args.jobs, specs, not args.no_roundtrip)
    totals = [Tally(label) for label, _ in specs]
    with mp.Pool(processes=args.jobs) as pool:
        for index, shard_tallies in enumerate(
            pool.imap_unordered(run_shard, jobs), start=1
        ):
            for total, shard in zip(totals, shard_tallies, strict=True):
                total.merge(shard)
            logging.info(
                "Shard %d/%d done (%.0fs elapsed, %s docs)",
                index,
                len(jobs),
                time.time() - started,
                f"{totals[0].documents:,}",
            )
    elapsed = time.time() - started

    payload = {
        "corpus": str(corpus),
        "corpus_fingerprint": corpus_fingerprint(corpus),
        "elapsed_seconds": round(elapsed, 1),
        "jobs": args.jobs,
        "roundtrip_stride": None if args.no_roundtrip else ROUNDTRIP_STRIDE,
        "ratio_bin_width": RATIO_BIN_WIDTH,
        "tokenizers": {total.label: total.summary() for total in totals},
    }
    output = resolve_under_repo(args.repo_root, args.output)
    write_json(output, payload)

    for total in totals:
        summary = total.summary()
        logging.info(
            "%s: %s docs, %s words, %s tokens, %.4f tokens/word, %.4f bytes/token",
            total.label,
            f"{summary['documents']:,}",
            f"{summary['words']:,}",
            f"{summary['total_tokens']:,}",
            summary["corpus_tokens_per_word"],
            summary["bytes_per_token"],
        )
    logging.info("Wrote %s in %.1fs", output, elapsed)


if __name__ == "__main__":
    main()
