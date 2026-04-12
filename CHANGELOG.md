# Changelog

All notable changes to SomNLP-Corpus are documented here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Rust workspace with `common` and `corpus-tools` crates
- Minimal `Document` record type (`text` + optional `source`)
- Public dataset downloaders: HPLT, CC100, mC4, OPUS, MADLAD, MT560
- `merge_corpora` tool to combine raw JSONL sources
- Documentation: architecture, pipeline, plan, roadmap, source catalog

### Planned

- Cleaning, deduplication, and language filtering pipeline
- Wikipedia and Somali web collectors
- Books, subtitles, OCR, and community contribution intake
- First corpus release (`v0.1.0`)
