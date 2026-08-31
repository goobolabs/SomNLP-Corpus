#!/usr/bin/env python3
"""Dual audit re-measurement for v0.2 corpus. Writes reports/06_cleaning_audit.md"""

from __future__ import annotations

import json
import re
from collections import Counter
from pathlib import Path

CORPUS = Path("data/final/final_so.jsonl")
REPORT = Path("reports/06_cleaning_audit.md")
FUNNEL = [
    "reports/01_merge_stats.json",
    "reports/02_clean_stats.json",
    "reports/03_lid_stats.json",
    "reports/04_deep_clean_stats.json",
    "reports/05_near_dedup_stats.json",
]

PATTERNS = {
    "url": re.compile(r"(?i)(https?://|www\.|[a-z0-9.-]+\.(com|org|net|so|edu|gov|info|io|co)\b)"),
    "email": re.compile(r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b"),
    "foreign_en": re.compile(r"(?i)\b(the|and|with|from|this|that|click here|contact us|written by)\b"),
    "escaped": re.compile(r"\\n|\\t"),
    "boilerplate": re.compile(r"(?i)(live help|contact us|click here|posted by|tags:)"),
    "html_tag": re.compile(r"</?[a-zA-Z][^>]*>"),
    "mojibake": re.compile(r"Ã|â€|â„"),
    "sentinel_url": re.compile(r"⟨url⟩"),
    "sentinel_email": re.compile(r"⟨email⟩"),
}


def load_funnel() -> list[tuple[str, int, int]]:
    rows = []
    for path in FUNNEL:
        p = Path(path)
        if not p.exists():
            continue
        data = json.loads(p.read_text())
        phase = data.get("phase", p.stem)
        inp = data.get("input_docs") or data.get("total_input_docs") or 0
        out = data.get("output_docs") or data.get("total_output_docs") or 0
        rows.append((phase, int(inp), int(out)))
    return rows


def scan_corpus(limit: int | None = None) -> tuple[int, Counter]:
    counts: Counter = Counter()
    total = 0
    with CORPUS.open() as f:
        for line in f:
            if limit is not None and total >= limit:
                break
            total += 1
            rec = json.loads(line)
            text = rec.get("text", "")
            for name, pat in PATTERNS.items():
                if pat.search(text):
                    counts[name] += 1
    return total, counts


def main() -> None:
    if not CORPUS.exists():
        raise SystemExit(f"Missing corpus: {CORPUS}")

    total, counts = scan_corpus()
    funnel = load_funnel()

    lines = [
        "# v0.2 cleaning audit",
        "",
        f"- Corpus: `{CORPUS}`",
        f"- Records scanned: **{total:,}**",
        "",
        "## Pipeline funnel",
        "",
        "| Stage | Input | Output |",
        "|-------|------:|-------:|",
    ]
    for phase, inp, out in funnel:
        lines.append(f"| {phase} | {inp:,} | {out:,} |")

    lines.extend(["", "## Defect detectors (full corpus)", "", "| Detector | Count | % |", "|----------|------:|--:|"])
    for name, _ in PATTERNS.items():
        c = counts[name]
        pct = 100.0 * c / total if total else 0.0
        lines.append(f"| {name} | {c:,} | {pct:.2f}% |")

    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text("\n".join(lines) + "\n")
    print(f"Wrote {REPORT}")


if __name__ == "__main__":
    main()
