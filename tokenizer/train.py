#!/usr/bin/env python3
"""Train a Somali Byte Pair Encoding tokenizer on the prepared corpus."""

from __future__ import annotations

import argparse
import logging
import time
from pathlib import Path
from typing import Iterator, Tuple

from tokenizers import Tokenizer, decoders, models, normalizers, pre_tokenizers, trainers

from common import (
    DEFAULT_RAW_CORPUS,
    DEFAULT_TOKENIZER,
    SPECIAL_TOKENS,
    add_repo_path_arg,
    fail,
    iter_corpus_lines,
    resolve_under_repo,
    setup_logging,
)

UNK_TOKEN = SPECIAL_TOKENS[0]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Train a Somali BPE tokenizer.")
    add_repo_path_arg(parser)
    parser.add_argument(
        "--corpus",
        type=Path,
        default=DEFAULT_RAW_CORPUS,
        help="Plain-text training corpus (one document per line)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_TOKENIZER,
        help="Output tokenizer JSON path",
    )
    parser.add_argument(
        "--vocab-size",
        type=int,
        default=32_000,
        help="Target vocabulary size including special tokens (default: 32000)",
    )
    parser.add_argument(
        "--min-frequency",
        type=int,
        default=2,
        help="Minimum pair frequency to merge (default: 2)",
    )
    parser.add_argument(
        "--limit-lines",
        type=int,
        default=None,
        help="Train on at most N corpus lines (smoke test)",
    )
    parser.add_argument("--verbose", action="store_true", help="Enable debug logging")
    return parser.parse_args()


def build_tokenizer(vocab_size: int, min_frequency: int) -> Tuple[Tokenizer, trainers.BpeTrainer]:
    tokenizer = Tokenizer(models.BPE(unk_token=UNK_TOKEN))
    tokenizer.normalizer = normalizers.NFC()
    tokenizer.pre_tokenizer = pre_tokenizers.Whitespace()
    tokenizer.decoder = decoders.BPEDecoder()

    trainer = trainers.BpeTrainer(
        vocab_size=vocab_size,
        min_frequency=min_frequency,
        show_progress=True,
        special_tokens=SPECIAL_TOKENS,
    )
    return tokenizer, trainer


def corpus_iterator(path: Path, limit: int | None) -> Iterator[str]:
    for line in iter_corpus_lines(path, limit=limit):
        yield line


def main() -> None:
    args = parse_args()
    setup_logging(args.verbose)

    corpus_path = resolve_under_repo(args.repo_root, args.corpus)
    output_path = resolve_under_repo(args.repo_root, args.output)

    if not corpus_path.is_file():
        fail(f"Training corpus not found: {corpus_path}. Run prepare_corpus.py first.")

    if args.vocab_size <= len(SPECIAL_TOKENS):
        fail(
            f"--vocab-size must exceed the number of special tokens ({len(SPECIAL_TOKENS)})."
        )

    logging.info("Training BPE on %s", corpus_path)
    logging.info("Target vocabulary size: %d", args.vocab_size)
    if args.limit_lines is not None:
        logging.info("Limiting training to %d lines", args.limit_lines)

    tokenizer, trainer = build_tokenizer(args.vocab_size, args.min_frequency)
    started = time.perf_counter()

    try:
        tokenizer.train_from_iterator(
            corpus_iterator(corpus_path, args.limit_lines),
            trainer=trainer,
        )
    except Exception as exc:  # noqa: BLE001 - surface library failures cleanly
        fail(f"BPE training failed: {exc}")

    output_path.parent.mkdir(parents=True, exist_ok=True)
    tokenizer.save(str(output_path))

    elapsed = time.perf_counter() - started
    logging.info("Saved tokenizer to %s", output_path)
    logging.info("Training completed in %.1fs", elapsed)
    logging.info("Vocabulary size: %d", tokenizer.get_vocab_size(with_added_tokens=True))


if __name__ == "__main__":
    main()
