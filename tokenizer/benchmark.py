#!/usr/bin/env python3
"""Benchmark Somali tokenizers on the held-out evaluation split.

Every tokenizer is scored on the same documents in a single streaming pass. Only the
per-document ratios are retained, never the document strings, so peak memory stays in the
tens of megabytes regardless of split size.
"""

from __future__ import annotations

import argparse
import json
import logging
import statistics
import time
import unicodedata
from collections import defaultdict
from collections.abc import Callable, Iterator
from dataclasses import dataclass, field
from pathlib import Path

from tokenizers import Tokenizer

from common import (
    DEFAULT_BENCHMARK,
    DEFAULT_EVAL_HOLDOUT,
    DEFAULT_TOKENIZER,
    DEFAULT_TOKENIZER_V2,
    add_repo_path_arg,
    count_words,
    fail,
    resolve_under_repo,
    setup_logging,
    write_json,
)

SAMPLE_SENTENCES = [
    "Soomaaliya waa dal ku yaal Geeska Afrika oo leh taariikh hodan ah.",
    "( Eebe wuxuu yidhi ) Waxaan ku dhaartay halkay Xiddiguhu ku dhacaan ( ku qarsoomaan ) .",
    "Waxaan u baahanahay inaan helno nidaam Garaad Gacmeed oo ku hadla Af-Soomaali si sax ah.",
    "Boggan wuxuu ka kooban yahay ⟨url⟩ iyo macluumaad ku saabsan caafimaadka bulshada.",
]

BASELINE_SPECS = [
    ("bert-base-uncased", "BERT-base"),
    ("xlm-roberta-base", "XLM-RoBERTa-base"),
]


@dataclass
class Accumulator:
    """Running per-tokenizer tallies. Holds floats and counters only, never texts."""

    label: str
    ratios: list[float] = field(default_factory=list)
    total_tokens: int = 0
    total_bytes: int = 0
    total_words: int = 0
    documents: int = 0
    roundtrip_failures: int = 0
    unk_documents: int = 0
    roundtrip_checked: int = 0
    by_source_tokens: dict[str, int] = field(default_factory=lambda: defaultdict(int))
    by_source_words: dict[str, int] = field(default_factory=lambda: defaultdict(int))

    def add(self, source: str, text: str, n_tokens: int) -> None:
        words = count_words(text)
        self.documents += 1
        self.total_tokens += n_tokens
        self.total_bytes += len(text.encode("utf-8"))
        self.total_words += words
        if words:
            self.ratios.append(n_tokens / words)
            self.by_source_tokens[source] += n_tokens
            self.by_source_words[source] += words

    def summary(self) -> dict[str, object]:
        ordered = sorted(self.ratios)
        if not ordered:
            return {"label": self.label, "documents": 0}
        p95_index = max(0, min(len(ordered) - 1, round(0.95 * (len(ordered) - 1))))
        return {
            "label": self.label,
            "documents": self.documents,
            "mean_tokens_per_word": round(statistics.mean(ordered), 4),
            "median_tokens_per_word": round(statistics.median(ordered), 4),
            "p95_tokens_per_word": round(ordered[p95_index], 4),
            "corpus_tokens_per_word": round(self.total_tokens / self.total_words, 4),
            "bytes_per_token": round(self.total_bytes / self.total_tokens, 4),
            "total_tokens": self.total_tokens,
            "roundtrip_checked": self.roundtrip_checked,
            "roundtrip_failures": self.roundtrip_failures,
            "roundtrip_fidelity": (
                round(1 - self.roundtrip_failures / self.roundtrip_checked, 6)
                if self.roundtrip_checked
                else None
            ),
            "unk_documents": self.unk_documents,
            "by_source_tokens_per_word": {
                source: round(self.by_source_tokens[source] / self.by_source_words[source], 4)
                for source in sorted(
                    self.by_source_words, key=lambda s: -self.by_source_words[s]
                )
            },
        }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Benchmark Somali tokenizers on a held-out split.")
    add_repo_path_arg(parser)
    parser.add_argument(
        "--eval",
        type=Path,
        default=DEFAULT_EVAL_HOLDOUT,
        help="Held-out evaluation JSONL from prepare_corpus.py",
    )
    parser.add_argument(
        "--tokenizer",
        action="append",
        default=None,
        metavar="LABEL=PATH",
        help="Tokenizer to score; repeatable. Defaults to v1 and v2.",
    )
    parser.add_argument(
        "--sweep-dir",
        type=Path,
        default=None,
        help="Also score every somali-bpe-v2-*.json in this directory",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="Score at most N evaluation documents",
    )
    parser.add_argument(
        "--roundtrip-limit",
        type=int,
        default=100_000,
        help="Verify decode(encode(x)) on the first N documents (default: 100000)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_BENCHMARK,
        help="Write benchmark JSON to this path",
    )
    parser.add_argument(
        "--no-baseline",
        action="store_true",
        help="Skip BERT / XLM-RoBERTa baseline comparison (also skipped when offline)",
    )
    parser.add_argument("--verbose", action="store_true", help="Enable debug logging")
    return parser.parse_args()


def iter_eval_docs(path: Path, limit: int | None = None) -> Iterator[tuple[str, str]]:
    """Yield (source, text) from the evaluation JSONL."""
    with path.open(encoding="utf-8") as handle:
        for index, line in enumerate(handle):
            if limit is not None and index >= limit:
                break
            line = line.strip()
            if not line:
                continue
            record = json.loads(line)
            yield record.get("source", "unknown"), record["text"]


def load_tokenizers(args: argparse.Namespace) -> list[tuple[str, Tokenizer]]:
    specs: list[tuple[str, Path]] = []
    if args.tokenizer:
        for entry in args.tokenizer:
            label, _, raw_path = entry.partition("=")
            if not raw_path:
                label, raw_path = Path(entry).stem, entry
            specs.append((label, resolve_under_repo(args.repo_root, Path(raw_path))))
    else:
        defaults = (
            ("v1-whitespace-32k", DEFAULT_TOKENIZER),
            ("v2-bytelevel", DEFAULT_TOKENIZER_V2),
        )
        for label, default in defaults:
            if default.is_file():
                specs.append((label, default))

    if args.sweep_dir:
        sweep_dir = resolve_under_repo(args.repo_root, args.sweep_dir)
        if not sweep_dir.is_dir():
            fail(f"Sweep directory not found: {sweep_dir}")
        candidates = sorted(sweep_dir.glob("somali-bpe-v2-*.json"))
        if not candidates:
            fail(f"No somali-bpe-v2-*.json candidates in {sweep_dir}")
        for path in candidates:
            specs.append((path.stem.replace("somali-bpe-", ""), path))

    loaded: list[tuple[str, Tokenizer]] = []
    for label, path in specs:
        if not path.is_file():
            fail(f"Tokenizer not found: {path}")
        loaded.append((label, Tokenizer.from_file(str(path))))
        logging.info("Loaded %s from %s (vocab=%d)", label, path, loaded[-1][1].get_vocab_size())
    if not loaded:
        fail("No tokenizers to benchmark. Train one first or pass --tokenizer LABEL=PATH.")
    return loaded


def load_baselines(skip: bool) -> list[tuple[str, Callable[[str], list[int]]]]:
    if skip:
        return []
    baselines: list[tuple[str, Callable[[str], list[int]]]] = []
    for model_id, label in BASELINE_SPECS:
        try:
            from transformers import AutoTokenizer

            hf = AutoTokenizer.from_pretrained(model_id, use_fast=True)
        except Exception as exc:  # noqa: BLE001 - baselines are optional and need network
            logging.warning("Baseline %s unavailable: %s", label, exc)
            continue
        baselines.append((label, lambda text, tok=hf: tok.encode(text, add_special_tokens=False)))
    return baselines


def print_sample(label: str, tokenizer: Tokenizer, text: str) -> None:
    encoding = tokenizer.encode(text)
    words = count_words(text)
    decoded = tokenizer.decode(encoding.ids, skip_special_tokens=False)
    print(f"\n--- {label} ---")
    print(f"Text:  {text}")
    print(f"Tokens ({len(encoding.tokens)}): {encoding.tokens}")
    print(f"Ratio: {len(encoding.tokens) / words:.3f} tokens/word")
    print(f"Round-trip exact: {decoded == unicodedata.normalize('NFC', text)}")


def main() -> None:
    args = parse_args()
    setup_logging(args.verbose)

    eval_path = resolve_under_repo(args.repo_root, args.eval)
    output_path = resolve_under_repo(args.repo_root, args.output)

    tokenizers = load_tokenizers(args)

    for label, tokenizer in tokenizers:
        for sentence in SAMPLE_SENTENCES:
            print_sample(label, tokenizer, sentence)

    if not eval_path.is_file():
        fail(f"Evaluation split not found: {eval_path}. Run prepare_corpus.py first.")

    baselines = load_baselines(args.no_baseline)
    accumulators = {label: Accumulator(label) for label, _ in tokenizers}
    accumulators.update({label: Accumulator(label) for label, _ in baselines})
    unk_ids = {
        label: tokenizer.token_to_id("<unk>") for label, tokenizer in tokenizers
    }

    logging.info("Scoring %s", eval_path)
    started = time.perf_counter()
    seen = 0

    for source, text in iter_eval_docs(eval_path, args.limit):
        seen += 1
        normalized = unicodedata.normalize("NFC", text)
        for label, tokenizer in tokenizers:
            encoding = tokenizer.encode(text)
            accumulator = accumulators[label]
            accumulator.add(source, text, len(encoding.ids))
            unk_id = unk_ids[label]
            if unk_id is not None and unk_id in encoding.ids:
                accumulator.unk_documents += 1
            if seen <= args.roundtrip_limit:
                accumulator.roundtrip_checked += 1
                if tokenizer.decode(encoding.ids, skip_special_tokens=False) != normalized:
                    accumulator.roundtrip_failures += 1
        for label, encode_fn in baselines:
            accumulators[label].add(source, text, len(encode_fn(text)))

    if seen == 0:
        fail(f"No evaluation documents read from {eval_path}")

    logging.info("Scored %d documents in %.1fs", seen, time.perf_counter() - started)

    results = {
        "eval_split": str(eval_path),
        "documents_scored": seen,
        "roundtrip_limit": args.roundtrip_limit,
        "tokenizers": {label: accumulators[label].summary() for label, _ in tokenizers},
        "baselines": {label: accumulators[label].summary() for label, _ in baselines},
    }
    write_json(output_path, results)

    header = (
        f"\n{'tokenizer':24s} {'mean':>7s} {'median':>7s} {'p95':>7s} "
        f"{'B/tok':>7s} {'unk docs':>9s} {'round-trip':>11s}"
    )
    print(header)
    for label, _ in tokenizers + baselines:
        summary = accumulators[label].summary()
        fidelity = summary.get("roundtrip_fidelity")
        print(
            f"{label:24s} {summary['mean_tokens_per_word']:7.4f} "
            f"{summary['median_tokens_per_word']:7.4f} {summary['p95_tokens_per_word']:7.4f} "
            f"{summary['bytes_per_token']:7.3f} {summary['unk_documents']:9d} "
            f"{('n/a' if fidelity is None else f'{fidelity:.6f}'):>11s}"
        )
    logging.info("Wrote benchmark results to %s", output_path)


if __name__ == "__main__":
    main()
