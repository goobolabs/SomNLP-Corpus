"""Unit tests for the shared corpus helpers."""

from __future__ import annotations

import json

import pytest

from common import (
    EVAL_SPLIT_BUCKETS,
    EVAL_SPLIT_MODULUS,
    clean_document,
    corpus_fingerprint,
    count_words,
    is_eval_doc,
    iter_jsonl_docs,
    iter_jsonl_texts,
)


def test_clean_document_applies_nfc() -> None:
    assert clean_document("ábc") == "ábc"


def test_clean_document_collapses_long_newline_runs() -> None:
    assert clean_document("one\n\n\n\n\ntwo") == "one\n\ntwo"


def test_clean_document_preserves_paragraph_breaks() -> None:
    assert clean_document("one\n\ntwo") == "one\n\ntwo"


@pytest.mark.parametrize("text", ["", "   ", "\n\n\n"])
def test_clean_document_rejects_blank(text: str) -> None:
    assert clean_document(text) == ""


def test_count_words_splits_on_any_whitespace() -> None:
    assert count_words("a  b\tc\nd") == 4


def _write_jsonl(path, records: list[str]) -> None:
    path.write_text("\n".join(records) + "\n", encoding="utf-8")


def test_iter_jsonl_docs_skips_malformed_and_untyped(tmp_path) -> None:
    path = tmp_path / "corpus.jsonl"
    _write_jsonl(
        path,
        [
            json.dumps({"id": "mc4:1", "text": "wanaagsan", "provenance": {"source": "mc4"}}),
            "{not json",
            json.dumps({"id": "mc4:2", "text": 42, "provenance": {"source": "mc4"}}),
            json.dumps({"id": "mc4:3", "text": "   ", "provenance": {"source": "mc4"}}),
            json.dumps({"id": "nllb:4", "text": "hadal", "provenance": {"source": "nllb"}}),
        ],
    )
    docs = list(iter_jsonl_docs(path))
    assert [(d.doc_id, d.source, d.text) for d in docs] == [
        ("mc4:1", "mc4", "wanaagsan"),
        ("nllb:4", "nllb", "hadal"),
    ]


def test_iter_jsonl_docs_raises_when_not_skipping(tmp_path) -> None:
    path = tmp_path / "corpus.jsonl"
    _write_jsonl(path, ["{not json"])
    with pytest.raises(ValueError, match="Malformed JSON"):
        list(iter_jsonl_docs(path, skip_malformed=False))


def test_iter_jsonl_docs_falls_back_when_provenance_missing(tmp_path) -> None:
    path = tmp_path / "corpus.jsonl"
    _write_jsonl(path, [json.dumps({"text": "qoraal"})])
    doc = next(iter(iter_jsonl_docs(path)))
    assert doc.source == "unknown"
    assert doc.doc_id == "unknown:1"


def test_iter_jsonl_texts_still_yields_plain_strings(tmp_path) -> None:
    """scripts/word_frequency_analysis.py depends on this signature."""
    path = tmp_path / "corpus.jsonl"
    _write_jsonl(path, [json.dumps({"id": "mc4:1", "text": "hal laba"})])
    assert list(iter_jsonl_texts(path)) == ["hal laba"]


def test_iter_jsonl_docs_honours_limit(tmp_path) -> None:
    path = tmp_path / "corpus.jsonl"
    _write_jsonl(path, [json.dumps({"id": f"mc4:{i}", "text": f"doc {i}"}) for i in range(10)])
    assert len(list(iter_jsonl_docs(path, limit=3))) == 3


def test_is_eval_doc_is_deterministic() -> None:
    assert is_eval_doc("mc4:abc123") == is_eval_doc("mc4:abc123")


def test_is_eval_doc_selects_expected_share() -> None:
    ids = [f"mc4:{i:08x}" for i in range(50_000)]
    share = sum(is_eval_doc(i) for i in ids) / len(ids)
    expected = EVAL_SPLIT_BUCKETS / EVAL_SPLIT_MODULUS
    assert expected * 0.8 < share < expected * 1.2


def test_corpus_fingerprint_detects_content_change(tmp_path) -> None:
    path = tmp_path / "corpus.jsonl"
    path.write_text("original\n", encoding="utf-8")
    before = corpus_fingerprint(path)
    path.write_text("changed content\n", encoding="utf-8")
    after = corpus_fingerprint(path)
    assert before["edge_sha256"] != after["edge_sha256"]
    assert before != after


def test_corpus_fingerprint_is_stable_for_unchanged_file(tmp_path) -> None:
    path = tmp_path / "corpus.jsonl"
    path.write_text("stable\n", encoding="utf-8")
    assert corpus_fingerprint(path) == corpus_fingerprint(path)
