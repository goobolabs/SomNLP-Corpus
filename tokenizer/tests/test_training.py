"""Properties of the training configuration itself."""

from __future__ import annotations

import json

import pytest
from tokenizers import pre_tokenizers

from common import SPECIAL_TOKENS_V2
from train import build_tokenizer, derive_tokenizer

# 256 byte symbols seeded via initial_alphabet, plus the reserved control tokens.
RESERVED_SLOTS = len(pre_tokenizers.ByteLevel.alphabet()) + len(SPECIAL_TOKENS_V2)


@pytest.fixture(scope="module")
def trained(fixture_texts):
    """Two tokenizers differing only in target vocabulary size."""

    def build(vocab_size: int, tmp):
        tokenizer, trainer = build_tokenizer(vocab_size, min_frequency=2)
        trainer.show_progress = False
        tokenizer.train_from_iterator(fixture_texts, trainer=trainer)
        path = tmp / f"v{vocab_size}.json"
        tokenizer.save(str(path))
        return tokenizer, json.loads(path.read_text(encoding="utf-8"))

    return build


def test_merge_count_is_vocab_size_minus_reserved(trained, tmp_path) -> None:
    _, config = trained(1_000, tmp_path)
    assert len(config["model"]["merges"]) == 1_000 - RESERVED_SLOTS


def test_merges_are_a_prefix_of_a_larger_run(trained, tmp_path) -> None:
    """Greedy BPE: merge k does not depend on the target size.

    This is what makes a vocabulary sweep interpretable -- the candidates differ only
    in where the merge list is cut, not in which merges were learned.
    """
    _, small = trained(1_000, tmp_path)
    _, large = trained(2_000, tmp_path)
    small_merges = small["model"]["merges"]
    assert large["model"]["merges"][: len(small_merges)] == small_merges


def test_special_tokens_occupy_the_lowest_ids(trained, tmp_path) -> None:
    tokenizer, _ = trained(1_000, tmp_path)
    assert [tokenizer.token_to_id(t) for t in SPECIAL_TOKENS_V2] == list(
        range(len(SPECIAL_TOKENS_V2))
    )


def test_derived_tokenizer_equals_a_natively_trained_one(trained, tmp_path, fixture_texts) -> None:
    """Truncating a larger run must reproduce the smaller run exactly.

    This is the property train.py --derive-from relies on to avoid re-reading the corpus
    once per candidate vocabulary size.
    """
    from tokenizers import Tokenizer

    trained(2_000, tmp_path)
    native, _ = trained(1_000, tmp_path)

    derived_path = tmp_path / "derived.json"
    derive_tokenizer(tmp_path / "v2000.json", 1_000, derived_path)
    derived = Tokenizer.from_file(str(derived_path))

    assert derived.get_vocab() == native.get_vocab()
    for text in fixture_texts:
        assert derived.encode(text).ids == native.encode(text).ids


def test_derive_rejects_growing_a_tokenizer(trained, tmp_path) -> None:
    trained(1_000, tmp_path)
    with pytest.raises(SystemExit):
        derive_tokenizer(tmp_path / "v1000.json", 2_000, tmp_path / "bigger.json")
