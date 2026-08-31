"""Shared helpers for the Somali BPE tokenizer pipeline."""

from __future__ import annotations

import argparse
import hashlib
import json
import logging
import re
import sys
import unicodedata
from collections.abc import Iterator
from pathlib import Path
from typing import Any, NamedTuple

TOKENIZER_DIR = Path(__file__).resolve().parent
REPO_ROOT = TOKENIZER_DIR.parent

DEFAULT_CORPUS_JSONL = REPO_ROOT / "data" / "final" / "final_so.jsonl"
DEFAULT_RAW_CORPUS = TOKENIZER_DIR / "somali_raw_corpus.txt"
DEFAULT_EVAL_HOLDOUT = TOKENIZER_DIR / "eval_holdout.jsonl"
DEFAULT_TOKENIZER = TOKENIZER_DIR / "somali-bpe-tokenizer.json"
DEFAULT_TOKENIZER_V2 = TOKENIZER_DIR / "somali-bpe-v2.json"
DEFAULT_SWEEP_DIR = TOKENIZER_DIR / "sweep"
DEFAULT_HF_EXPORT_DIR = TOKENIZER_DIR / "v2"
DEFAULT_STATS = TOKENIZER_DIR / "tokenizer_stats.json"
DEFAULT_BENCHMARK = TOKENIZER_DIR / "benchmark_results.json"

# v1 (legacy, BERT-style) special tokens. Kept so the shipped v1 artifact stays
# reproducible; new training uses SPECIAL_TOKENS_V2.
SPECIAL_TOKENS = ["<unk>", "[CLS]", "[SEP]", "<pad>", "[MASK]"]

# v2 (decoder-LM) special tokens. ByteLevel BPE cannot emit an unknown token, so
# there is no <unk>. The reserved block keeps the embedding table a fixed size
# when chat-template control tokens are added later.
SPECIAL_TOKENS_V2 = [
    "<|endoftext|>",
    "<|pad|>",
    "<|im_start|>",
    "<|im_end|>",
] + [f"<|reserved_{i}|>" for i in range(12)]

# Deterministic train/eval split: hash the record id and hold out
# EVAL_SPLIT_BUCKETS out of EVAL_SPLIT_MODULUS (0.8%, ~49k of 6.15M documents).
# Hash-based so the split needs no stored index and is reproducible from the id alone.
EVAL_SPLIT_MODULUS = 1000
EVAL_SPLIT_BUCKETS = 8

# Collapse three or more consecutive newlines to a paragraph break.
_MULTI_NEWLINE = re.compile(r"\n{3,}")

# Bytes read from each end of the corpus when fingerprinting it.
_FINGERPRINT_EDGE_BYTES = 64 * 1024


class CorpusDoc(NamedTuple):
    """One cleaned document plus the provenance needed for splitting and stats."""

    doc_id: str
    source: str
    text: str


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


def is_eval_doc(doc_id: str) -> bool:
    """Deterministically assign a record id to the held-out evaluation split."""
    digest = hashlib.blake2b(doc_id.encode("utf-8"), digest_size=8).digest()
    return int.from_bytes(digest, "big") % EVAL_SPLIT_MODULUS < EVAL_SPLIT_BUCKETS


def corpus_fingerprint(path: Path) -> dict[str, Any]:
    """Cheap identity for a large corpus file: size, mtime, and hashed edges.

    Reading both ends rather than the whole 6 GB file keeps this near-instant while
    still detecting a regenerated or truncated corpus.
    """
    stat = path.stat()
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        digest.update(handle.read(_FINGERPRINT_EDGE_BYTES))
        if stat.st_size > _FINGERPRINT_EDGE_BYTES:
            handle.seek(max(0, stat.st_size - _FINGERPRINT_EDGE_BYTES))
            digest.update(handle.read(_FINGERPRINT_EDGE_BYTES))
    return {
        "path": str(path),
        "size_bytes": stat.st_size,
        "mtime_ns": stat.st_mtime_ns,
        "edge_sha256": digest.hexdigest(),
    }


def iter_jsonl_docs(
    path: Path,
    *,
    limit: int | None = None,
    skip_malformed: bool = True,
) -> Iterator[CorpusDoc]:
    """Stream cleaned documents with provenance from a CorpusRecord JSONL file."""
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
            if not cleaned:
                continue
            provenance = record.get("provenance")
            source = "unknown"
            if isinstance(provenance, dict) and isinstance(provenance.get("source"), str):
                source = provenance["source"]
            doc_id = record.get("id")
            if not isinstance(doc_id, str):
                doc_id = f"{source}:{line_no}"
            yield CorpusDoc(doc_id=doc_id, source=source, text=cleaned)
    if malformed:
        logging.info("Skipped %d malformed or empty records in %s", malformed, path)


def iter_jsonl_texts(
    path: Path,
    *,
    limit: int | None = None,
    skip_malformed: bool = True,
) -> Iterator[str]:
    """Stream non-empty cleaned texts from a JSONL file with a ``text`` field.

    Retained as the stable public helper: scripts/word_frequency_analysis.py imports it.
    """
    for doc in iter_jsonl_docs(path, limit=limit, skip_malformed=skip_malformed):
        yield doc.text


def iter_corpus_lines(path: Path, *, limit: int | None = None) -> Iterator[str]:
    """Stream non-empty lines from a plain-text corpus file."""
    with path.open(encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if limit is not None and line_no > limit:
                break
            text = line.rstrip("\n")
            if text:
                yield text


def iter_corpus_batches(
    path: Path,
    *,
    limit: int | None = None,
    batch_size: int = 10_000,
) -> Iterator[list[str]]:
    """Stream the corpus in batches of lines.

    ``train_from_iterator`` reacquires the GIL for every item a Python generator yields,
    which makes line-at-a-time feeding single-thread bound on a multi-GB corpus. Handing
    the Rust trainer whole batches lets it parallelise across cores.
    """
    batch: list[str] = []
    for text in iter_corpus_lines(path, limit=limit):
        batch.append(text)
        if len(batch) >= batch_size:
            yield batch
            batch = []
    if batch:
        yield batch


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def fail(message: str, code: int = 1) -> None:
    logging.error(message)
    sys.exit(code)
