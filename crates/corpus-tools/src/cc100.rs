use std::io::Read;

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use xz2::read::XzDecoder;

const READ_CHUNK_SIZE: usize = 1024 * 1024;

pub fn iter_documents_from_url(
    client: &Client,
    url: &str,
    stream: bool,
) -> Result<impl Iterator<Item = Result<String>>> {
    if stream {
        let mut response = client
            .get(url)
            .send()
            .context("downloading CC-100 archive")?
            .error_for_status()
            .context("CC-100 download failed")?;

        let total = response.content_length();
        let progress = ProgressBar::new(total.unwrap_or(0));
        progress.set_style(
            ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes}")
                .context("progress template")?
                .progress_chars("=>-"),
        );
        progress.set_message("Downloading".to_string());

        let mut decoder = XzDecoder::new(&mut response);
        let mut buffer = Vec::new();
        let mut chunk = vec![0u8; READ_CHUNK_SIZE];
        loop {
            let read = decoder
                .read(&mut chunk)
                .context("decompressing CC-100 archive")?;
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            progress.inc(read as u64);
        }
        progress.finish_and_clear();

        Ok(parse_documents(buffer).into_iter().map(Ok))
    } else {
        let temp = tempfile::Builder::new().suffix(".txt.xz").tempfile()?;
        let path = temp.path().to_path_buf();
        std::fs::write(&path, client.get(url).send()?.bytes()?)?;
        let file = std::fs::File::open(&path)?;
        let mut decoder = XzDecoder::new(file);
        let mut buffer = Vec::new();
        decoder.read_to_end(&mut buffer)?;
        Ok(parse_documents(buffer).into_iter().map(Ok))
    }
}

fn parse_documents(bytes: Vec<u8>) -> Vec<String> {
    let mut documents = Vec::new();
    let mut start = 0usize;
    while let Some(rel) = bytes[start..].windows(2).position(|window| window == b"\n\n") {
        let end = start + rel;
        if let Some(text) = normalize_document(&bytes[start..end]) {
            documents.push(text);
        }
        start = end + 2;
    }
    if start < bytes.len() {
        if let Some(text) = normalize_document(&bytes[start..]) {
            documents.push(text);
        }
    }
    documents
}

fn normalize_document(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    let joined = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let joined = joined.trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}
