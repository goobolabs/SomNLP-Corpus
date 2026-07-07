"""Shared helpers for the Somali BPE tokenizer pipeline."""

from __future__ import annotations

import argparse
import json
import logging
import re
import sys
import unicodedata
from pathlib import Path
from typing import Any, Iterator

TOKENIZER_DIR = Path(__file__).resolve().parent
REPO_ROOT = TOKENIZER_DIR.parent

DEFAULT_CORPUS_JSONL = REPO_ROOT / "data" / "final" / "final_so.jsonl"
DEFAULT_RAW_CORPUS = TOKENIZER_DIR / "somali_raw_corpus.txt"
DEFAULT_TOKENIZER = TOKENIZER_DIR / "somali-bpe-tokenizer.json"
DEFAULT_STATS = TOKENIZER_DIR / "tokenizer_stats.json"
DEFAULT_BENCHMARK = TOKENIZER_DIR / "benchmark_results.json"

SPECIAL_TOKENS = [
    "<" + "unk" + ">",
    "[CLS]",
    "[SEP]",
    "<" + "pad" + ">",
    "[MASK]",
]

# Collapse three or more consecutive newlines to a paragraph break.
_MULTI_NEWLINE = re.compile(r"\n{3,}")


def setup_logging(verbose: bool = False) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(asctime)s [%(levelname)s] %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )


def add_repo_path_arg(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=REPO_ROOT,
        help=f"Repository root (default: {REPO_ROOT})",
    )


def resolve_under_repo(repo_root: Path, path: Path) -> Path:
    return path if path.is_absolute() else (repo_root / path).resolve()


def count_words(text: str) -> int:
    return len(text.split())


def clean_document(text: str) -> str:
    """Light tokenizer-oriented normalization for a single document."""
    text = unicodedata.normalize("NFC", text.strip())
    if not text:
        return ""
    text = _MULTI_NEWLINE.sub("\n\n", text)
    return text.strip()


def iter_jsonl_texts(
    path: Path,
    *,
    limit: int | None = None,
    skip_malformed: bool = True,
) -> Iterator[str]:
    """Stream non-empty cleaned texts from a JSONL file with a ``text`` field."""
    malformed = 0
    with path.open(encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if limit is not None and line_no > limit:
                break
            line = line.strip()
            if not line:
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError as exc:
                malformed += 1
                if skip_malformed:
                    logging.warning("Skipping malformed JSON at %s:%d (%s)", path, line_no, exc)
                    continue
                raise ValueError(f"Malformed JSON at {path}:{line_no}") from exc
            raw = record.get("text")
            if not isinstance(raw, str):
                malformed += 1
                logging.warning("Skipping record without string 'text' at %s:%d", path, line_no)
                continue
            cleaned = clean_document(raw)
            if cleaned:
                yield cleaned
    if malformed:
        logging.info("Skipped %d malformed or empty records in %s", malformed, path)


def iter_corpus_lines(path: Path, *, limit: int | None = None) -> Iterator[str]:
    """Stream non-empty lines from a plain-text corpus file."""
    with path.open(encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if limit is not None and line_no > limit:
                break
            text = line.rstrip("\n")
            if text:
                yield text


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def fail(message: str, code: int = 1) -> None:
    logging.error(message)
    sys.exit(code)
