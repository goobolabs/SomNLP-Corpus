"""Checks on the shipped tokenizer artifacts (v1 kept as-is, v2 held to the v2 contract)."""

from __future__ import annotations

import json

import pytest
from tokenizers import Tokenizer

from common import DEFAULT_TOKENIZER, DEFAULT_TOKENIZER_V2, SPECIAL_TOKENS_V2

v1_only = pytest.mark.skipif(not DEFAULT_TOKENIZER.is_file(), reason="v1 artifact not present")
v2_only = pytest.mark.skipif(
    not DEFAULT_TOKENIZER_V2.is_file(), reason="v2 artifact not trained yet"
)


@v1_only
def test_v1_still_loads() -> None:
    tokenizer = Tokenizer.from_file(str(DEFAULT_TOKENIZER))
    assert tokenizer.get_vocab_size() == 32_000


@v1_only
def test_v1_config_is_unchanged() -> None:
    """v1 is frozen for reproducibility, defects included; v2 is where fixes land."""
    config = json.loads(DEFAULT_TOKENIZER.read_text(encoding="utf-8"))
    assert config["pre_tokenizer"]["type"] == "Whitespace"
    assert config["decoder"]["type"] == "BPEDecoder"
    assert config["model"]["unk_token"] == "<unk>"


@v2_only
def test_v2_has_bytelevel_pipeline() -> None:
    config = json.loads(DEFAULT_TOKENIZER_V2.read_text(encoding="utf-8"))
    assert config["normalizer"]["type"] == "NFC"
    assert config["pre_tokenizer"]["type"] == "ByteLevel"
    assert config["pre_tokenizer"]["add_prefix_space"] is False
    assert config["decoder"]["type"] == "ByteLevel"
    assert config["post_processor"]["type"] == "ByteLevel"
    assert config["model"]["unk_token"] is None


@v2_only
def test_v2_carries_every_special_token() -> None:
    tokenizer = Tokenizer.from_file(str(DEFAULT_TOKENIZER_V2))
    for token in SPECIAL_TOKENS_V2:
        assert tokenizer.token_to_id(token) is not None, token


@v2_only
def test_v2_roundtrips_a_real_sentence() -> None:
    tokenizer = Tokenizer.from_file(str(DEFAULT_TOKENIZER_V2))
    text = "Soomaaliya waa dal ku yaal Geeska Afrika, oo leh taariikh hodan ah."
    assert tokenizer.decode(tokenizer.encode(text).ids, skip_special_tokens=False) == text
