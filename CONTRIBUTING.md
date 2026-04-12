# Contributing

Thank you for helping build Somali NLP resources. The most valuable contributions
right now are **bug reports on downloaders**, **new Somali data sources**, and
**cleaning rules** for the processing pipeline (planned in [ROADMAP.md](ROADMAP.md)).

## Before you start

Read [PLAN.md](PLAN.md), [ROADMAP.md](ROADMAP.md), and [docs/SOURCES.md](docs/SOURCES.md).
Open an issue for anything non-trivial before a large change.

## Development setup

Requires Rust 1.75+.

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

Release binaries:

```bash
cargo build --release
./target/release/download_hplt_so --limit 10
./target/release/merge_corpora --sources hplt --limit 50
```

## Ways to contribute

1. **Propose a data source** — see the source template in [docs/SOURCES.md](docs/SOURCES.md).
2. **Improve downloaders** — faster streaming, better error messages, new public datasets.
3. **Build pipeline stages** — cleaning, deduplication, language filtering (Phase 3 in the roadmap).
4. **Add Somali web collectors** — Wikipedia connector, news site scrapers with robots.txt compliance.
5. **Report data quality issues** — wrong language, encoding problems, empty shards.

## Rust code standards

- No `unwrap()` on fallible paths in library code; use `anyhow::Result` or typed errors.
- Run `cargo fmt` and `cargo clippy` before submitting.
- Keep binaries thin; put logic in `corpus-tools` library modules.
- Add tests when changing parsing, JSONL handling, or export logic.

## Proposing a new source

1. Open an issue titled `Source: <name>`.
2. Describe: access method, estimated volume, content domain, and any access restrictions.
3. For web sources: confirm robots.txt / terms allow collection.
4. Once agreed, add a downloader or collector and update [docs/SOURCES.md](docs/SOURCES.md).

## Commit guidelines

- Small, focused commits with a clear message.
- Update docs in the same change when behavior or sources change.
- Do not commit `data/` artifacts or `target/` build output.
