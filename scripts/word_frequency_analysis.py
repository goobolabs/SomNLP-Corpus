#!/usr/bin/env python3
"""Word and name frequency analysis over the final Somali corpus.

Streams data/final/final_so.jsonl once, tokenizes each document's `text`
field, and counts word frequencies plus matches against a curated Somali
given-name gazetteer. Writes reports/07_word_frequency.md and
reports/07_word_frequency_stats.json.
"""

from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT))

from tokenizer.common import DEFAULT_CORPUS_JSONL, iter_jsonl_texts, write_json  # noqa: E402

NAMES_PATH = Path(__file__).resolve().parent / "data" / "somali_names.txt"
STOPWORDS_PATH = Path(__file__).resolve().parent / "data" / "somali_stopwords.txt"
REPORT_MD = REPO_ROOT / "reports" / "07_word_frequency.md"
REPORT_JSON = REPO_ROOT / "reports" / "07_word_frequency_stats.json"

TOKEN_RE = re.compile(r"[a-z']+")

TOP_WORDS_FOR_REPORT = 50
TOP_N_HEADLINE = 10


def load_wordlist(path: Path) -> set[str]:
    return {line.strip() for line in path.read_text(encoding="utf-8").splitlines() if line.strip()}


def tokenize(text: str) -> list[str]:
    tokens = []
    for match in TOKEN_RE.finditer(text.lower()):
        token = match.group().strip("'")
        if token:
            tokens.append(token)
    return tokens


def scan_corpus(corpus_path: Path, names: set[str]) -> tuple[int, Counter, Counter]:
    word_counts: Counter = Counter()
    name_counts: Counter = Counter()
    docs = 0
    for docs, text in enumerate(iter_jsonl_texts(corpus_path), start=1):
        tokens = tokenize(text)
        word_counts.update(tokens)
        for token in tokens:
            if token in names:
                name_counts[token] += 1
        if docs % 200_000 == 0:
            print(f"  scanned {docs:,} docs...", file=sys.stderr)
    return docs, word_counts, name_counts


def render_table(rows: list[tuple[str, int]], headers: tuple[str, str]) -> list[str]:
    lines = [f"| {headers[0]} | {headers[1]} |", "|---|---:|"]
    for term, count in rows:
        lines.append(f"| {term} | {count:,} |")
    return lines


def main() -> None:
    if not DEFAULT_CORPUS_JSONL.exists():
        raise SystemExit(f"Missing corpus: {DEFAULT_CORPUS_JSONL}")
    if not NAMES_PATH.exists():
        raise SystemExit(f"Missing name gazetteer: {NAMES_PATH}")

    names = load_wordlist(NAMES_PATH)
    stopwords = load_wordlist(STOPWORDS_PATH)
    print(f"Loaded {len(names)} gazetteer names, {len(stopwords)} stopwords")
    print(f"Scanning {DEFAULT_CORPUS_JSONL} ...")

    docs, word_counts, name_counts = scan_corpus(DEFAULT_CORPUS_JSONL, names)
    total_tokens = sum(word_counts.values())
    unique_tokens = len(word_counts)

    content_counts = Counter({w: c for w, c in word_counts.items() if w not in stopwords})

    top_words_report = word_counts.most_common(TOP_WORDS_FOR_REPORT)
    top_content_report = content_counts.most_common(TOP_WORDS_FOR_REPORT)
    top_content_headline = content_counts.most_common(TOP_N_HEADLINE)
    top_names = name_counts.most_common(TOP_N_HEADLINE)

    print("\nTop 10 content words (stopwords removed):")
    for word, count in top_content_headline:
        print(f"  {word:<20} {count:,}")

    print("\nTop 10 names:")
    for name, count in top_names:
        print(f"  {name:<20} {count:,}")

    lines = [
        "# Word & name frequency analysis",
        "",
        f"- Corpus: `{DEFAULT_CORPUS_JSONL.relative_to(REPO_ROOT)}`",
        f"- Documents scanned: **{docs:,}**",
        f"- Total tokens: **{total_tokens:,}**",
        f"- Unique tokens: **{unique_tokens:,}**",
        f"- Gazetteer size: **{len(names)}** names ({NAMES_PATH.relative_to(REPO_ROOT)})",
        f"- Stopword list size: **{len(stopwords)}** ({STOPWORDS_PATH.relative_to(REPO_ROOT)})",
        "",
        "## Top 10 content words (stopwords removed)",
        "",
        *render_table(top_content_headline, ("word", "count")),
        "",
        f"## Top {TOP_WORDS_FOR_REPORT} content words (reference, stopwords removed)",
        "",
        *render_table(top_content_report, ("word", "count")),
        "",
        f"## Top {TOP_WORDS_FOR_REPORT} most frequent words, unfiltered (reference)",
        "",
        *render_table(top_words_report, ("word", "count")),
        "",
        "## Top 10 most frequent names (gazetteer match)",
        "",
        *render_table(top_names, ("name", "count")),
        "",
    ]

    REPORT_MD.parent.mkdir(parents=True, exist_ok=True)
    REPORT_MD.write_text("\n".join(lines) + "\n", encoding="utf-8")
    write_json(
        REPORT_JSON,
        {
            "corpus": str(DEFAULT_CORPUS_JSONL.relative_to(REPO_ROOT)),
            "documents_scanned": docs,
            "total_tokens": total_tokens,
            "unique_tokens": unique_tokens,
            "gazetteer_size": len(names),
            "stopword_count": len(stopwords),
            "top_words_unfiltered": top_words_report,
            "top_content_words": top_content_report,
            "top_names": top_names,
        },
    )
    print(f"\nWrote {REPORT_MD}")
    print(f"Wrote {REPORT_JSON}")


if __name__ == "__main__":
    main()
