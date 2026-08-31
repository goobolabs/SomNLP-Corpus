"""Shared fixtures. Tests train a tiny tokenizer so they stay fast and hermetic."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

TOKENIZER_DIR = Path(__file__).resolve().parent.parent
if str(TOKENIZER_DIR) not in sys.path:
    sys.path.insert(0, str(TOKENIZER_DIR))

FIXTURE_CORPUS = Path(__file__).resolve().parent / "fixtures" / "mini_corpus.jsonl"

# Small enough to train in well under a second, large enough to exercise real merges.
FIXTURE_VOCAB_SIZE = 2_000


@pytest.fixture(scope="session")
def fixture_docs() -> list[dict[str, str]]:
    with FIXTURE_CORPUS.open(encoding="utf-8") as handle:
        return [json.loads(line) for line in handle if line.strip()]


@pytest.fixture(scope="session")
def fixture_texts(fixture_docs: list[dict[str, str]]) -> list[str]:
    return [doc["text"] for doc in fixture_docs]


@pytest.fixture(scope="session")
def mini_tokenizer(fixture_texts: list[str]):
    """A ByteLevel BPE trained on the fixture, using the real production builder."""
    from train import build_tokenizer

    tokenizer, trainer = build_tokenizer(FIXTURE_VOCAB_SIZE, min_frequency=2)
    trainer.show_progress = False
    tokenizer.train_from_iterator(fixture_texts, trainer=trainer)
    return tokenizer
