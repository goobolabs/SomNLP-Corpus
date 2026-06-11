//! QuranEnc Somali (Yacob Yusuf) translation downloader.
//!
//! Fetches all 114 surahs from the QuranEnc API and splits each verse into
//! clean translation text and standalone footnote explanations. Verse numbers
//! and inline footnote markers are stripped from both outputs.

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;

pub const SOURCE_TAG: &str = "quran";

const TRANSLATION: &str = "somali_yacob";
const SURA_COUNT: u32 = 114;

#[derive(Debug, Deserialize)]
struct SuraResponse {
    result: Vec<Verse>,
}

#[derive(Debug, Deserialize)]
struct Verse {
    #[serde(default)]
    translation: String,
    #[serde(default)]
    footnotes: String,
}

/// Cleaned translation and footnote corpora in canonical (surah, verse) order.
#[derive(Debug, Default)]
pub struct QuranCorpus {
    pub translations: Vec<String>,
    pub footnotes: Vec<String>,
}

pub fn sura_url(sura: u32) -> String {
    format!("https://quranenc.com/api/v1/translation/sura/{TRANSLATION}/{sura}")
}

pub fn source_url() -> String {
    format!("https://quranenc.com/api/v1/translation/sura/{TRANSLATION} (suras 1-{SURA_COUNT})")
}

async fn fetch_sura(client: &Client, sura: u32) -> Result<Vec<Verse>> {
    let response = client
        .get(sura_url(sura))
        .send()
        .await
        .with_context(|| format!("fetching surah {sura}"))?
        .error_for_status()
        .with_context(|| format!("surah {sura} request failed"))?;
    let body: SuraResponse = response
        .json()
        .await
        .with_context(|| format!("parsing surah {sura} JSON"))?;
    Ok(body.result)
}

/// Fetch all 114 surahs concurrently and return cleaned translations and
/// footnotes.
pub async fn fetch_corpus(client: &Client, concurrency: usize) -> Result<QuranCorpus> {
    let cleaner = Cleaner::new()?;
    let progress = ProgressBar::new(SURA_COUNT as u64);
    progress.set_style(
        ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {pos}/{len} suras")
            .context("progress template")?
            .progress_chars("=>-"),
    );
    progress.set_message("Fetching");

    let suras: Vec<Result<Vec<Verse>>> = stream::iter(1..=SURA_COUNT)
        .map(|sura| fetch_sura(client, sura))
        .buffered(concurrency)
        .inspect(|_| progress.inc(1))
        .collect()
        .await;
    progress.finish_and_clear();

    let mut corpus = QuranCorpus::default();
    for sura in suras {
        for verse in sura? {
            if let Some(text) = cleaner.translation(&verse.translation) {
                corpus.translations.push(text);
            }
            if let Some(text) = cleaner.footnote(&verse.footnotes) {
                corpus.footnotes.push(text);
            }
        }
    }
    Ok(corpus)
}

/// Compiled-once regexes for stripping verse numbers and footnote markers.
struct Cleaner {
    leading_number: Regex,
    inline_ref: Regex,
    leading_marker: Regex,
}

impl Cleaner {
    fn new() -> Result<Self> {
        Ok(Self {
            leading_number: Regex::new(r"^\s*\d+\.\s*").context("leading number regex")?,
            inline_ref: Regex::new(r"\s*\[\d+\]").context("inline footnote ref regex")?,
            leading_marker: Regex::new(r"^\s*\[\d+\]\.\s*").context("footnote marker regex")?,
        })
    }

    fn translation(&self, raw: &str) -> Option<String> {
        let without_number = self.leading_number.replace(raw, "");
        let without_refs = self.inline_ref.replace_all(&without_number, "");
        normalize(&without_refs)
    }

    fn footnote(&self, raw: &str) -> Option<String> {
        if raw.trim().is_empty() {
            return None;
        }
        let without_marker = self.leading_marker.replace(raw, "");
        normalize(&without_marker)
    }
}

fn normalize(text: &str) -> Option<String> {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        None
    } else {
        Some(collapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cleaner() -> Cleaner {
        Cleaner::new().unwrap()
    }

    #[test]
    fn strips_leading_verse_number() {
        assert_eq!(
            cleaner()
                .translation("3. Naxariistaha (Naxariis guud ahaaneed), Naxariista badan.")
                .unwrap(),
            "Naxariistaha (Naxariis guud ahaaneed), Naxariista badan."
        );
    }

    #[test]
    fn strips_inline_footnote_ref() {
        assert_eq!(
            cleaner()
                .translation("2. Boqorka Maalinta Jaazeynta [2].")
                .unwrap(),
            "Boqorka Maalinta Jaazeynta."
        );
    }

    #[test]
    fn strips_footnote_marker() {
        assert_eq!(
            cleaner()
                .footnote("[2]. Qiyaamada ee iska leh wax walba.")
                .unwrap(),
            "Qiyaamada ee iska leh wax walba."
        );
    }

    #[test]
    fn empty_footnote_is_skipped() {
        assert!(cleaner().footnote("").is_none());
        assert!(cleaner().footnote("   ").is_none());
    }
}
