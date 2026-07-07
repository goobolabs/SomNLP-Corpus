#!/usr/bin/env python3
"""Validate the trained Somali BPE tokenizer and benchmark token efficiency."""

from __future__ import annotations

import argparse
import logging
import random
import statistics
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from tokenizers import Tokenizer

from common import (
    DEFAULT_BENCHMARK,
    DEFAULT_RAW_CORPUS,
    DEFAULT_TOKENIZER,
    add_repo_path_arg,
    count_words,
    fail,
    iter_corpus_lines,
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

RESERVOIR_SIZE = 10_000


@dataclass(frozen=True)
class RatioStats:
    mean: float
    median: float
    p95: float
    samples: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Test and benchmark the Somali BPE tokenizer.")
    add_repo_path_arg(parser)
    parser.add_argument(
        "--tokenizer",
        type=Path,
        default=DEFAULT_TOKENIZER,
        help="Path to somali-bpe-tokenizer.json",
    )
    parser.add_argument(
        "--corpus",
        type=Path,
        default=DEFAULT_RAW_CORPUS,
        help="Plain-text corpus for sampling benchmarks",
    )
    parser.add_argument(
        "--sample-size",
        type=int,
        default=RESERVOIR_SIZE,
        help="Number of corpus lines to benchmark (default: 10000)",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=42,
        help="Random seed for corpus sampling",
    )
    parser.add_argument(
        "--benchmark-output",
        type=Path,
        default=DEFAULT_BENCHMARK,
        help="Write benchmark JSON to this path",
    )
    parser.add_argument(
        "--no-baseline",
        action="store_true",
        help="Skip BERT / XLM-RoBERTa baseline comparison",
    )
    parser.add_argument("--verbose", action="store_true", help="Enable debug logging")
    return parser.parse_args()


def load_native_tokenizer(path: Path) -> Tokenizer:
    if not path.is_file():
        fail(f"Tokenizer not found: {path}. Run train.py first.")
    return Tokenizer.from_file(str(path))


def encode_with_hf(model_name: str):
    from transformers import AutoTokenizer

    tokenizer = AutoTokenizer.from_pretrained(model_name, use_fast=True)
    return tokenizer


def token_word_ratio(text: str, encode_fn: Callable[[str], list[int]]) -> float | None:
    words = count_words(text)
    if words == 0:
        return None
    tokens = encode_fn(text)
    if not tokens:
        return None
    return len(tokens) / words


def reservoir_sample(path: Path, sample_size: int, seed: int) -> list[str]:
    rng = random.Random(seed)
    reservoir: list[str] = []
    for index, line in enumerate(iter_corpus_lines(path)):
        if index < sample_size:
            reservoir.append(line)
        else:
            pick = rng.randint(0, index)
            if pick < sample_size:
                reservoir[pick] = line
    return reservoir


def summarize_ratios(ratios: list[float]) -> RatioStats:
    ordered = sorted(ratios)
    p95_index = max(0, min(len(ordered) - 1, int(round(0.95 * (len(ordered) - 1)))))
    return RatioStats(
        mean=statistics.mean(ordered),
        median=statistics.median(ordered),
        p95=ordered[p95_index],
        samples=len(ordered),
    )


def benchmark_tokenizer(name: str, texts: list[str], encode_fn: Callable[[str], list[int]]) -> RatioStats:
    ratios: list[float] = []
    for text in texts:
        ratio = token_word_ratio(text, encode_fn)
        if ratio is not None:
            ratios.append(ratio)
    if not ratios:
        fail(f"No valid ratios computed for tokenizer: {name}")
    stats = summarize_ratios(ratios)
    logging.info(
        "%s token/word ratio — mean=%.3f median=%.3f p95=%.3f (n=%d)",
        name,
        stats.mean,
        stats.median,
        stats.p95,
        stats.samples,
    )
    return stats


def print_sample(tokenizer: Tokenizer, text: str) -> None:
    encoding = tokenizer.encode(text)
    tokens = encoding.tokens
    ids = encoding.ids
    words = count_words(text)
    ratio = len(tokens) / words if words else float("nan")

    print("\n--- Sample encoding ---")
    print(f"Text: {text}")
    print(f"Words: {words}")
    print(f"Tokens ({len(tokens)}): {tokens}")
    print(f"Token IDs: {ids}")
    print(f"Token-to-word ratio: {ratio:.3f}")


def main() -> None:
    args = parse_args()
    setup_logging(args.verbose)

    tokenizer_path = resolve_under_repo(args.repo_root, args.tokenizer)
    corpus_path = resolve_under_repo(args.repo_root, args.corpus)
    benchmark_path = resolve_under_repo(args.repo_root, args.benchmark_output)

    native = load_native_tokenizer(tokenizer_path)

    for sentence in SAMPLE_SENTENCES:
        print_sample(native, sentence)

    if not corpus_path.is_file():
        logging.warning("Corpus not found at %s; skipping benchmark sampling.", corpus_path)
        return

    logging.info("Sampling up to %d lines from %s", args.sample_size, corpus_path)
    started = time.perf_counter()
    sample_texts = reservoir_sample(corpus_path, args.sample_size, args.seed)
    logging.info("Collected %d sample lines in %.1fs", len(sample_texts), time.perf_counter() - started)

    native_stats = benchmark_tokenizer(
        "Somali BPE",
        sample_texts,
        lambda text: native.encode(text).ids,
    )

    results: dict[str, object] = {
        "sample_size_requested": args.sample_size,
        "sample_size_used": len(sample_texts),
        "seed": args.seed,
        "native_bpe": {
            "mean": round(native_stats.mean, 4),
            "median": round(native_stats.median, 4),
            "p95": round(native_stats.p95, 4),
            "samples": native_stats.samples,
        },
        "baselines": {},
    }

    if not args.no_baseline:
        baseline_specs = [
            ("bert-base-uncased", "BERT-base"),
            ("xlm-roberta-base", "XLM-RoBERTa-base"),
        ]
        for model_id, label in baseline_specs:
            try:
                hf_tokenizer = encode_with_hf(model_id)
                stats = benchmark_tokenizer(
                    label,
                    sample_texts,
                    lambda text, tok=hf_tokenizer: tok.encode(text, add_special_tokens=False),
                )
                results["baselines"][label] = {
                    "model_id": model_id,
                    "mean": round(stats.mean, 4),
                    "median": round(stats.median, 4),
                    "p95": round(stats.p95, 4),
                    "samples": stats.samples,
                }
            except Exception as exc:  # noqa: BLE001 - optional baseline download
                logging.warning("Baseline %s unavailable: %s", label, exc)

    write_json(benchmark_path, results)
    logging.info("Wrote benchmark results to %s", benchmark_path)


if __name__ == "__main__":
    main()
