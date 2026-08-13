//! Tanzil Somali (Mahmud Muhammad Abduh) Qur'an translation downloader.
//!
//! Tanzil publishes exactly one Somali translation, `so.abduh`, as a single
//! UTF-8 file: one ayah per line as `sura|aya|text`, closed by a `#`-prefixed
//! license trailer. It carries no footnotes — it is a different translator from
//! the QuranEnc source in [`crate::quran`], not another view of it — so this
//! downloader has a single output.
//!
//! Nothing is cleaned here beyond splitting off the line prefix: like every
//! other downloader in this crate the output is raw source text, and
//! normalization is the corpus-pipeline's job.

use std::path::Path;

use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;

use crate::jsonl::{is_non_empty, JsonlWriter};
use crate::Stats;

pub const SOURCE_TAG: &str = "quran-tanzil";
pub const TRANSLATION_ID: &str = "so.abduh";
pub const TRANSLATOR: &str = "Mahmud Muhammad Abduh";

/// Ayahs in the Qur'an. Tanzil ships every one, so a short parse means the
/// download was truncated rather than that the edition differs.
pub const AYA_COUNT: usize = 6236;

const USER_AGENT: &str = "corpus-tools/0.1";

/// Tanzil serves the numbered (`sura|aya|text`) variant behind `type=txt-2`;
/// `agree=true` is the terms acknowledgement its download form submits.
pub fn download_url(translation_id: &str) -> String {
    format!("https://tanzil.net/trans/?transID={translation_id}&type=txt-2&agree=true")
}

/// Human-readable provenance line for the export summary.
pub fn source_url() -> String {
    format!("https://tanzil.net/trans/ (translation: {TRANSLATION_ID}, {TRANSLATOR})")
}

/// One line of the Tanzil file. Only `text` is exported; the numbers exist so a
/// truncated download can be caught before anything is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ayah {
    pub sura: u16,
    pub aya: u16,
    pub text: String,
}

/// Parse the whole file, skipping blank lines and the trailing license block.
pub fn parse_translation(body: &str) -> Result<Vec<Ayah>> {
    let mut ayahs = Vec::with_capacity(AYA_COUNT);
    for (index, raw) in body.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let ayah = parse_line(line).with_context(|| format!("line {}", index + 1))?;
        ayahs.push(ayah);
    }
    Ok(ayahs)
}

fn parse_line(line: &str) -> Result<Ayah> {
    // splitn(3) so a `|` inside the translation stays part of the text.
    let mut fields = line.splitn(3, '|');
    let (Some(sura), Some(aya), Some(text)) = (fields.next(), fields.next(), fields.next()) else {
        bail!("expected 'sura|aya|text', found {line:?}");
    };
    Ok(Ayah {
        sura: sura
            .trim()
            .parse()
            .with_context(|| format!("sura number {sura:?}"))?,
        aya: aya
            .trim()
            .parse()
            .with_context(|| format!("aya number {aya:?}"))?,
        text: text.trim().to_string(),
    })
}

fn verify_complete(ayahs: &[Ayah]) -> Result<()> {
    if ayahs.len() != AYA_COUNT {
        bail!(
            "expected {AYA_COUNT} ayahs but parsed {}; the download looks truncated",
            ayahs.len()
        );
    }
    Ok(())
}

/// Fetch and parse the translation, rejecting an incomplete download.
pub fn fetch_translation(client: &Client, url: &str) -> Result<Vec<Ayah>> {
    let body = client
        .get(url)
        .send()
        .context("requesting the Tanzil translation")?
        .error_for_status()
        .context("Tanzil download failed")?
        .text()
        .context("reading the Tanzil response body")?;
    let ayahs = parse_translation(&body)?;
    verify_complete(&ayahs)?;
    Ok(ayahs)
}

/// Download the Tanzil Somali translation and export one JSONL record per ayah,
/// carrying the verse as `text`.
pub fn download_tanzil(output: &Path, url: &str, limit: Option<u64>) -> Result<Stats> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("building the HTTP client")?;
    let ayahs = fetch_translation(&client, url)?;

    let mut writer = JsonlWriter::create(output, "Writing")?;
    let mut written = 0u64;
    for ayah in &ayahs {
        if limit.is_some_and(|limit| written >= limit) {
            break;
        }
        if !is_non_empty(&ayah.text) {
            continue;
        }
        writer.write_text(&ayah.text)?;
        written += 1;
    }
    let stats = writer.stats.clone();
    writer.finish();
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
1|1|Magaca Eebe yaan kubillaabaynaa ee Naxariis guud iyo mid gaaraba Naxariista.
1|2|Mahad Eebaa iska leh ee barbaariyaha Caalamka ah (Koonka).

# --------------------------------------------------------------------
#
#  Quran Translation
#  Name: Abduh
#  ID: so.abduh
#  Source: Tanzil.net
#
# --------------------------------------------------------------------
";

    #[test]
    fn parses_numbered_lines() {
        let ayahs = parse_translation(SAMPLE).unwrap();
        assert_eq!(ayahs.len(), 2);
        assert_eq!(ayahs[1].sura, 1);
        assert_eq!(ayahs[1].aya, 2);
        assert_eq!(
            ayahs[1].text,
            "Mahad Eebaa iska leh ee barbaariyaha Caalamka ah (Koonka)."
        );
    }

    #[test]
    fn skips_blank_lines_and_license_trailer() {
        assert!(parse_translation(SAMPLE)
            .unwrap()
            .iter()
            .all(|ayah| !ayah.text.starts_with('#')));
    }

    #[test]
    fn keeps_pipe_inside_translation() {
        let ayahs = parse_translation("2|1|Alif | Laam | Miim.").unwrap();
        assert_eq!(ayahs[0].text, "Alif | Laam | Miim.");
    }

    #[test]
    fn rejects_malformed_line() {
        assert!(parse_translation("1|1").is_err());
        assert!(parse_translation("x|1|Magaca Eebe.").is_err());
    }

    #[test]
    fn truncated_download_is_rejected() {
        let ayahs = parse_translation(SAMPLE).unwrap();
        assert!(verify_complete(&ayahs).is_err());
    }

    #[test]
    fn download_url_targets_the_numbered_format() {
        assert_eq!(
            download_url(TRANSLATION_ID),
            "https://tanzil.net/trans/?transID=so.abduh&type=txt-2&agree=true"
        );
    }
}
