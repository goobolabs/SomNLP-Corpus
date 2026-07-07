#!/usr/bin/env python3
"""Extract Somali text from the release corpus into a plain-text training file."""

from __future__ import annotations

import argparse
import logging
import time
from pathlib import Path

from tqdm import tqdm

from common import (
    DEFAULT_CORPUS_JSONL,
    DEFAULT_RAW_CORPUS,
    DEFAULT_STATS,
    add_repo_path_arg,
    count_words,
    fail,
    iter_jsonl_texts,
    resolve_under_repo,
    setup_logging,
    write_json,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Prepare a plain-text Somali corpus for BPE tokenizer training.",
    )
    add_repo_path_arg(parser)
    parser.add_argument(
        "--input",
        type=Path,
        default=DEFAULT_CORPUS_JSONL,
        help="Source JSONL corpus (default: data/final/final_so.jsonl)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_RAW_CORPUS,
        help="Output plain-text file (one document per line)",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="Process at most N JSONL records (for smoke tests)",
    )
    parser.add_argument(
        "--stats",
        type=Path,
        nargs="?",
        const=DEFAULT_STATS,
        default=None,
        help="Write JSON summary stats (default path: tokenizer_stats.json)",
    )
    parser.add_argument("--verbose", action="store_true", help="Enable debug logging")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    setup_logging(args.verbose)

    input_path = resolve_under_repo(args.repo_root, args.input)
    output_path = resolve_under_repo(args.repo_root, args.output)

    if not input_path.is_file():
        fail(f"Input corpus not found: {input_path}")

    output_path.parent.mkdir(parents=True, exist_ok=True)

    logging.info("Reading from %s", input_path)
    logging.info("Writing to %s", output_path)

    docs = 0
    chars = 0
    words = 0
    started = time.perf_counter()

    with output_path.open("w", encoding="utf-8", newline="\n") as out:
        for text in tqdm(
            iter_jsonl_texts(input_path, limit=args.limit),
            desc="Preparing corpus",
            unit="doc",
        ):
            out.write(text)
            out.write("\n")
            docs += 1
            chars += len(text)
            words += count_words(text)

    elapsed = time.perf_counter() - started
    if docs == 0:
        fail(f"No documents written from {input_path}")

    logging.info(
        "Finished: %d documents, %d words, %d characters in %.1fs",
        docs,
        words,
        chars,
        elapsed,
    )

    if args.stats is not None:
        stats_path = resolve_under_repo(args.repo_root, args.stats)
        write_json(
            stats_path,
            {
                "input": str(input_path),
                "output": str(output_path),
                "documents": docs,
                "words": words,
                "characters": chars,
                "elapsed_seconds": round(elapsed, 2),
            },
        )
        logging.info("Wrote stats to %s", stats_path)


if __name__ == "__main__":
    main()
