#!/usr/bin/env python3
"""Extract Somali text from the release corpus into tokenizer training material.

Writes two artifacts:

* ``somali_raw_corpus.txt`` -- training text. Paragraph structure is preserved, so a
  multi-paragraph document spans several lines. That is deliberate: BPE learns from word
  frequencies, which paragraph splitting does not change, and an LM benefits from seeing
  real newlines. It does mean the file is *not* one document per line.
* ``eval_holdout.jsonl`` -- the held-out evaluation split, one JSON record per line
  (``id``, ``source``, ``text``). JSONL rather than plain text so that documents
  containing newlines stay exactly one record per line and keep their provenance.
"""

from __future__ import annotations

import argparse
import json
import logging
import time
from collections import defaultdict
from pathlib import Path

from tqdm import tqdm

from common import (
    DEFAULT_CORPUS_JSONL,
    DEFAULT_EVAL_HOLDOUT,
    DEFAULT_RAW_CORPUS,
    DEFAULT_STATS,
    EVAL_SPLIT_BUCKETS,
    EVAL_SPLIT_MODULUS,
    add_repo_path_arg,
    corpus_fingerprint,
    count_words,
    fail,
    is_eval_doc,
    iter_jsonl_docs,
    resolve_under_repo,
    setup_logging,
    write_json,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Prepare Somali training text and an eval holdout for BPE training.",
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
        help="Output training text file",
    )
    parser.add_argument(
        "--eval-output",
        type=Path,
        default=DEFAULT_EVAL_HOLDOUT,
        help="Output held-out evaluation JSONL",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="Process at most N JSONL records (for smoke tests)",
    )
    parser.add_argument(
        "--no-holdout",
        action="store_true",
        help="Write every document to the training file (no evaluation split)",
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


class SplitCounter:
    """Per-source document, word, character and multi-line tallies for one split."""

    def __init__(self) -> None:
        self.docs: dict[str, int] = defaultdict(int)
        self.words: dict[str, int] = defaultdict(int)
        self.chars: dict[str, int] = defaultdict(int)
        self.multiline: dict[str, int] = defaultdict(int)

    def add(self, source: str, text: str) -> None:
        self.docs[source] += 1
        self.words[source] += count_words(text)
        self.chars[source] += len(text)
        if "\n" in text:
            self.multiline[source] += 1

    @property
    def total_docs(self) -> int:
        return sum(self.docs.values())

    @property
    def total_words(self) -> int:
        return sum(self.words.values())

    def summary(self) -> dict[str, object]:
        total_docs = self.total_docs
        total_words = self.total_words
        by_source = {
            source: {
                "documents": self.docs[source],
                "document_share": round(self.docs[source] / total_docs, 6) if total_docs else 0.0,
                "words": self.words[source],
                "word_share": round(self.words[source] / total_words, 6) if total_words else 0.0,
                "characters": self.chars[source],
                "words_per_document": round(self.words[source] / self.docs[source], 2),
                "multiline_documents": self.multiline[source],
            }
            for source in sorted(self.docs, key=lambda s: -self.words[s])
        }
        return {
            "documents": total_docs,
            "words": total_words,
            "characters": sum(self.chars.values()),
            "multiline_documents": sum(self.multiline.values()),
            "by_source": by_source,
        }


def main() -> None:
    args = parse_args()
    setup_logging(args.verbose)

    input_path = resolve_under_repo(args.repo_root, args.input)
    output_path = resolve_under_repo(args.repo_root, args.output)
    eval_path = resolve_under_repo(args.repo_root, args.eval_output)

    if not input_path.is_file():
        fail(f"Input corpus not found: {input_path}")

    output_path.parent.mkdir(parents=True, exist_ok=True)
    eval_path.parent.mkdir(parents=True, exist_ok=True)

    logging.info("Reading from %s", input_path)
    logging.info("Writing training text to %s", output_path)
    if args.no_holdout:
        logging.info("Holdout disabled; all documents go to the training file")
    else:
        logging.info(
            "Writing eval holdout to %s (%d/%d of ids)",
            eval_path,
            EVAL_SPLIT_BUCKETS,
            EVAL_SPLIT_MODULUS,
        )

    train = SplitCounter()
    evaluation = SplitCounter()
    started = time.perf_counter()

    with (
        output_path.open("w", encoding="utf-8", newline="\n") as train_out,
        eval_path.open("w", encoding="utf-8", newline="\n") as eval_out,
    ):
        for doc in tqdm(
            iter_jsonl_docs(input_path, limit=args.limit),
            desc="Preparing corpus",
            unit="doc",
        ):
            if not args.no_holdout and is_eval_doc(doc.doc_id):
                evaluation.add(doc.source, doc.text)
                eval_out.write(
                    json.dumps(
                        {"id": doc.doc_id, "source": doc.source, "text": doc.text},
                        ensure_ascii=False,
                    )
                )
                eval_out.write("\n")
                continue
            train.add(doc.source, doc.text)
            train_out.write(doc.text)
            train_out.write("\n")

    elapsed = time.perf_counter() - started
    if train.total_docs == 0:
        fail(f"No training documents written from {input_path}")

    logging.info(
        "Train split: %d documents, %d words. Eval split: %d documents, %d words. (%.1fs)",
        train.total_docs,
        train.total_words,
        evaluation.total_docs,
        evaluation.total_words,
        elapsed,
    )

    if args.stats is not None:
        stats_path = resolve_under_repo(args.repo_root, args.stats)
        write_json(
            stats_path,
            {
                "input": str(input_path),
                "train_output": str(output_path),
                "eval_output": str(eval_path),
                "corpus_fingerprint": corpus_fingerprint(input_path),
                "eval_split": {
                    "modulus": EVAL_SPLIT_MODULUS,
                    "buckets": EVAL_SPLIT_BUCKETS,
                    "hash": "blake2b-8(id)",
                    "enabled": not args.no_holdout,
                },
                "documents": train.total_docs + evaluation.total_docs,
                "words": train.total_words + evaluation.total_words,
                "train": train.summary(),
                "eval": evaluation.summary(),
                "elapsed_seconds": round(elapsed, 2),
            },
        )
        logging.info("Wrote stats to %s", stats_path)


if __name__ == "__main__":
    main()
