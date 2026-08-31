#!/usr/bin/env python3
"""Train Somali ByteLevel BPE tokenizers on the prepared corpus.

v2 uses a ByteLevel pre-tokenizer and decoder rather than v1's whitespace split. That
choice is what makes ``decode(encode(x))`` reproduce the input exactly and makes an
unknown token structurally impossible: every byte is already in the alphabet.
"""

from __future__ import annotations

import argparse
import json
import logging
import time
from pathlib import Path

from tokenizers import (
    Tokenizer,
    decoders,
    models,
    normalizers,
    pre_tokenizers,
    processors,
    trainers,
)

from common import (
    DEFAULT_CORPUS_JSONL,
    DEFAULT_RAW_CORPUS,
    DEFAULT_STATS,
    DEFAULT_SWEEP_DIR,
    DEFAULT_TOKENIZER_V2,
    SPECIAL_TOKENS_V2,
    add_repo_path_arg,
    corpus_fingerprint,
    fail,
    iter_corpus_batches,
    read_json,
    resolve_under_repo,
    setup_logging,
)

DEFAULT_VOCAB_SIZE = 48_000


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Train Somali ByteLevel BPE tokenizer(s).")
    add_repo_path_arg(parser)
    parser.add_argument(
        "--corpus",
        type=Path,
        default=DEFAULT_RAW_CORPUS,
        help="Plain-text training corpus",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_TOKENIZER_V2,
        help="Output tokenizer JSON path (single-size runs only)",
    )
    parser.add_argument(
        "--vocab-size",
        type=int,
        default=DEFAULT_VOCAB_SIZE,
        help=f"Target vocabulary size including special tokens (default: {DEFAULT_VOCAB_SIZE})",
    )
    parser.add_argument(
        "--sweep",
        type=str,
        default=None,
        help="Comma-separated vocab sizes to train, e.g. 16384,32000,48000,65536. "
        "Writes one tokenizer per size into --sweep-dir instead of --output.",
    )
    parser.add_argument(
        "--derive-from",
        type=Path,
        default=None,
        help="Truncate this already-trained tokenizer to the --sweep / --vocab-size targets "
        "instead of retraining. Exact for BPE (merges are a prefix of a larger run).",
    )
    parser.add_argument(
        "--sweep-dir",
        type=Path,
        default=DEFAULT_SWEEP_DIR,
        help="Directory for sweep outputs",
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
    parser.add_argument(
        "--stats",
        type=Path,
        default=DEFAULT_STATS,
        help="tokenizer_stats.json used for the corpus freshness check",
    )
    parser.add_argument(
        "--source-corpus",
        type=Path,
        default=DEFAULT_CORPUS_JSONL,
        help="Release JSONL the training text should derive from (freshness check)",
    )
    parser.add_argument(
        "--allow-stale",
        action="store_true",
        help="Train even if the training text does not match the current release corpus",
    )
    parser.add_argument("--verbose", action="store_true", help="Enable debug logging")
    return parser.parse_args()


def build_tokenizer(vocab_size: int, min_frequency: int) -> tuple[Tokenizer, trainers.BpeTrainer]:
    """ByteLevel BPE: lossless round-trip, no unknown token, 256-byte seed alphabet."""
    tokenizer = Tokenizer(models.BPE())
    tokenizer.normalizer = normalizers.NFC()
    tokenizer.pre_tokenizer = pre_tokenizers.ByteLevel(add_prefix_space=False, use_regex=True)
    tokenizer.decoder = decoders.ByteLevel()
    tokenizer.post_processor = processors.ByteLevel(trim_offsets=True)

    trainer = trainers.BpeTrainer(
        vocab_size=vocab_size,
        min_frequency=min_frequency,
        show_progress=True,
        special_tokens=SPECIAL_TOKENS_V2,
        initial_alphabet=pre_tokenizers.ByteLevel.alphabet(),
    )
    return tokenizer, trainer


def check_corpus_freshness(stats_path: Path, source_path: Path, allow_stale: bool) -> None:
    """Refuse to train on training text that predates the current release corpus.

    Without this the previous pipeline would silently retrain on a months-old
    somali_raw_corpus.txt, because train.py only checked that the file existed.
    """
    if not stats_path.is_file():
        message = (
            f"No {stats_path.name} found; cannot verify the training text matches "
            f"{source_path}. Run prepare_corpus.py --stats first."
        )
        if allow_stale:
            logging.warning("%s (continuing: --allow-stale)", message)
            return
        fail(f"{message} Pass --allow-stale to train anyway.")

    recorded = read_json(stats_path).get("corpus_fingerprint")
    if not recorded:
        message = f"{stats_path.name} has no corpus_fingerprint; re-run prepare_corpus.py --stats."
        if allow_stale:
            logging.warning("%s (continuing: --allow-stale)", message)
            return
        fail(f"{message} Pass --allow-stale to train anyway.")

    if not source_path.is_file():
        logging.warning("Release corpus %s not found; skipping freshness check.", source_path)
        return

    current = corpus_fingerprint(source_path)
    if current != recorded:
        differing = [k for k in current if current[k] != recorded.get(k)]
        message = (
            f"Training text is stale: {source_path} no longer matches the fingerprint "
            f"recorded in {stats_path.name} (differs on: {', '.join(differing)}). "
            "Re-run prepare_corpus.py --stats."
        )
        if allow_stale:
            logging.warning("%s (continuing: --allow-stale)", message)
            return
        fail(f"{message} Pass --allow-stale to train anyway.")

    logging.info("Corpus freshness check passed against %s", source_path)


def train_one(
    corpus_path: Path, output_path: Path, vocab_size: int, args: argparse.Namespace
) -> None:
    logging.info("Training ByteLevel BPE (vocab=%d) on %s", vocab_size, corpus_path)
    tokenizer, trainer = build_tokenizer(vocab_size, args.min_frequency)
    started = time.perf_counter()

    try:
        tokenizer.train_from_iterator(
            iter_corpus_batches(corpus_path, limit=args.limit_lines),
            trainer=trainer,
        )
    except Exception as exc:  # noqa: BLE001 - surface library failures cleanly
        fail(f"BPE training failed: {exc}")

    output_path.parent.mkdir(parents=True, exist_ok=True)
    tokenizer.save(str(output_path))

    logging.info(
        "Saved vocab=%d to %s in %.1fs (actual vocab size: %d)",
        vocab_size,
        output_path,
        time.perf_counter() - started,
        tokenizer.get_vocab_size(with_added_tokens=True),
    )


def derive_tokenizer(source_path: Path, target_vocab: int, output_path: Path) -> None:
    """Produce a smaller tokenizer by truncating a larger one's merge list.

    BPE merges are learned greedily, so merge k is independent of the target vocabulary
    size: a larger run's merge list is a superset of a smaller run's, and cutting it at
    the right point reproduces the smaller run exactly. Deriving avoids re-reading the
    corpus once per candidate size, which dominates sweep cost. See
    tests/test_training.py::test_merges_are_a_prefix_of_a_larger_run.
    """
    config = json.loads(source_path.read_text(encoding="utf-8"))
    model = config["model"]
    source_vocab_size = len(model["vocab"])
    reserved = source_vocab_size - len(model["merges"])

    if target_vocab > source_vocab_size:
        fail(
            f"Cannot derive vocab {target_vocab} from a {source_vocab_size}-token tokenizer; "
            "train the larger size first."
        )
    if target_vocab <= reserved:
        fail(f"--vocab-size {target_vocab} must exceed the {reserved} non-merge tokens.")

    model["vocab"] = {
        token: index for token, index in model["vocab"].items() if index < target_vocab
    }
    model["merges"] = model["merges"][: target_vocab - reserved]
    config["added_tokens"] = [
        token for token in config.get("added_tokens", []) if token["id"] < target_vocab
    ]

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(config, ensure_ascii=False), encoding="utf-8")
    logging.info(
        "Derived vocab=%d from %s -> %s (%d merges kept of %d)",
        target_vocab,
        source_path.name,
        output_path,
        len(model["merges"]),
        source_vocab_size - reserved,
    )


def main() -> None:
    args = parse_args()
    setup_logging(args.verbose)

    corpus_path = resolve_under_repo(args.repo_root, args.corpus)
    stats_path = resolve_under_repo(args.repo_root, args.stats)
    source_path = resolve_under_repo(args.repo_root, args.source_corpus)

    if not corpus_path.is_file():
        fail(f"Training corpus not found: {corpus_path}. Run prepare_corpus.py first.")

    check_corpus_freshness(stats_path, source_path, args.allow_stale)

    if args.sweep:
        try:
            sizes = [int(part) for part in args.sweep.split(",") if part.strip()]
        except ValueError:
            fail(f"--sweep must be comma-separated integers, got: {args.sweep!r}")
        if not sizes:
            fail("--sweep listed no vocabulary sizes")
    else:
        sizes = [args.vocab_size]

    for vocab_size in sizes:
        if vocab_size <= len(SPECIAL_TOKENS_V2) + len(pre_tokenizers.ByteLevel.alphabet()):
            fail(
                f"--vocab-size {vocab_size} must exceed the {len(SPECIAL_TOKENS_V2)} special "
                f"tokens plus the 256-byte initial alphabet."
            )

    sweep_dir = resolve_under_repo(args.repo_root, args.sweep_dir)
    derive_source = (
        resolve_under_repo(args.repo_root, args.derive_from) if args.derive_from else None
    )
    if derive_source and not derive_source.is_file():
        fail(f"--derive-from tokenizer not found: {derive_source}")

    for vocab_size in sizes:
        if args.sweep:
            output_path = sweep_dir / f"somali-bpe-v2-{vocab_size}.json"
        else:
            output_path = resolve_under_repo(args.repo_root, args.output)
        if derive_source:
            derive_tokenizer(derive_source, vocab_size, output_path)
        else:
            train_one(corpus_path, output_path, vocab_size, args)


if __name__ == "__main__":
    main()
