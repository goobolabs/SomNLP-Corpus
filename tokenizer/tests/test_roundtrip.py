"""The properties that motivated v2: exact round-trip and no unknown token."""

from __future__ import annotations

import unicodedata

import pytest
from tokenizers import pre_tokenizers

from common import SPECIAL_TOKENS_V2

# Inputs that broke v1 or that stress the byte alphabet.
ADVERSARIAL = [
    "Soomaaliya waa dal ku yaal Geeska Afrika, oo leh taariikh hodan ah.",
    "  leading and trailing spaces  ",
    "tabs\there\tand\tthere",
    "double  spaces   and    more",
    "line one\nline two\n\nparagraph break",
    "emoji 🙂 and 中文 and runes ᚠᚢᚦ",
    "⟨url⟩ iyo ⟨email⟩ sentinels",
    "reer binu Israa 'iil",
    "Af-Soomaali iyo Garaad Gacmeed",
    "digits 1234567890 and punctuation !?;:.,",
    "",
    "   ",
    "\n",
]


def _roundtrips(tokenizer, text: str) -> bool:
    encoding = tokenizer.encode(text)
    decoded = tokenizer.decode(encoding.ids, skip_special_tokens=False)
    return decoded == unicodedata.normalize("NFC", text)


@pytest.mark.parametrize("text", ADVERSARIAL)
def test_adversarial_strings_roundtrip(mini_tokenizer, text: str) -> None:
    assert _roundtrips(mini_tokenizer, text)


def test_every_fixture_document_roundtrips(mini_tokenizer, fixture_texts) -> None:
    failures = [text for text in fixture_texts if not _roundtrips(mini_tokenizer, text)]
    assert failures == [], f"{len(failures)} of {len(fixture_texts)} documents did not round-trip"


def test_vocab_has_no_unknown_token(mini_tokenizer) -> None:
    """v1 could emit <unk>; ByteLevel makes that structurally impossible."""
    vocab = mini_tokenizer.get_vocab()
    assert "<unk>" not in vocab
    assert mini_tokenizer.token_to_id("<unk>") is None


def test_unseen_scripts_produce_no_unknown_token(mini_tokenizer) -> None:
    # v1 encoded this exact string as ['<unk>', '<unk>', '<unk>'].
    encoding = mini_tokenizer.encode("ᚠᚢᚦ")
    assert encoding.ids
    assert "<unk>" not in encoding.tokens
    assert _roundtrips(mini_tokenizer, "ᚠᚢᚦ")


def test_byte_alphabet_is_complete(mini_tokenizer) -> None:
    vocab = mini_tokenizer.get_vocab()
    missing = [c for c in pre_tokenizers.ByteLevel.alphabet() if c not in vocab]
    assert missing == []


@pytest.mark.parametrize("token", SPECIAL_TOKENS_V2)
def test_special_tokens_are_single_ids(mini_tokenizer, token: str) -> None:
    encoding = mini_tokenizer.encode(token)
    assert encoding.tokens == [token]
    assert mini_tokenizer.decode(encoding.ids, skip_special_tokens=False) == token


def test_special_tokens_are_stripped_by_default(mini_tokenizer) -> None:
    """Documented behaviour: decode() drops control tokens unless asked to keep them."""
    ids = mini_tokenizer.encode("<|endoftext|>").ids
    assert mini_tokenizer.decode(ids) == ""
    assert mini_tokenizer.decode(ids, skip_special_tokens=False) == "<|endoftext|>"


def test_reserved_slots_exist_for_future_control_tokens(mini_tokenizer) -> None:
    assert mini_tokenizer.token_to_id("<|reserved_0|>") is not None
    assert mini_tokenizer.token_to_id("<|reserved_11|>") is not None


def test_decomposed_input_is_normalized_not_preserved(mini_tokenizer) -> None:
    """Round-trip is exact with respect to NFC, which the corpus already is."""
    decomposed = "á"
    ids = mini_tokenizer.encode(decomposed).ids
    decoded = mini_tokenizer.decode(ids, skip_special_tokens=False)
    assert decoded == "á"
    assert decoded == unicodedata.normalize("NFC", decomposed)
