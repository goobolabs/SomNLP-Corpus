//! Fetch the single English–Somali archive from the official AllenAI bucket.
//!
//! The download is resumable (HTTP `Range`), verified (the finished file must
//! decompress), and atomic (`.part` until it is complete). Every redirect hop
//! is re-checked against [`is_official_host`], so the archive can only ever
//! come from the official bucket.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, CONTENT_LENGTH, CONTENT_RANGE, LOCATION, RANGE};
use reqwest::{StatusCode, Url};

use crate::nllb::config::is_official_host;
use crate::nllb::export::PART_SUFFIX;

/// Bytes decompressed when checking that an archive is readable.
const VERIFY_BYTES: usize = 1024 * 1024;

/// Transfer buffer size.
const CHUNK_SIZE: usize = 1024 * 1024;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// Knobs for [`download_archive`].
#[derive(Debug, Clone, Copy)]
pub struct DownloadOptions {
    /// Re-download even when a valid archive is already on disk.
    pub overwrite: bool,
    pub show_progress: bool,
    pub max_redirects: usize,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            overwrite: false,
            show_progress: true,
            max_redirects: 5,
        }
    }
}

/// Download `url` to `destination`, returning the completed path.
///
/// A partially downloaded `.part` file is resumed rather than restarted, and
/// is left in place when the download fails so the next run can continue from
/// where it stopped.
pub fn download_archive(
    url: &str,
    destination: &Path,
    options: &DownloadOptions,
) -> Result<PathBuf> {
    if !is_official_host(url) {
        bail!("refusing non-official host: {url}");
    }

    if destination.exists() && !options.overwrite {
        if verify_gzip(destination) {
            println!(
                "Valid archive already present, skipping download: {}",
                destination.display()
            );
            return Ok(destination.to_path_buf());
        }
        println!(
            "Existing file is not a valid gzip, re-downloading: {}",
            destination.display()
        );
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }

    let part = part_path(destination);
    if options.overwrite {
        let _ = fs::remove_file(&part);
    }
    let mut existing = part.metadata().map(|meta| meta.len()).unwrap_or(0);

    let client = Client::builder()
        .user_agent("corpus-tools/0.1")
        // Redirects are followed by hand so each hop can be host-checked.
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        // On the blocking client this is applied per read/write, not to the
        // request as a whole, so a slow multi-GB transfer is not cut short.
        .timeout(READ_TIMEOUT)
        .build()
        .context("building HTTP client")?;

    let mut response = send_with_redirects(&client, url, existing, options.max_redirects)?;

    let mut append = true;
    let total = match response.status() {
        StatusCode::PARTIAL_CONTENT => range_total(response.headers(), existing),
        StatusCode::OK => {
            if existing > 0 {
                println!("Server ignored the Range request; restarting the download.");
                existing = 0;
                append = false;
            }
            content_length(response.headers())
        }
        StatusCode::RANGE_NOT_SATISFIABLE => {
            // Nothing left to fetch: the part file is already the whole thing.
            if part.exists() {
                fs::rename(&part, destination).with_context(|| {
                    format!("renaming {} to {}", part.display(), destination.display())
                })?;
            }
            if !verify_gzip(destination) {
                bail!("completed file failed gzip verification");
            }
            return Ok(destination.to_path_buf());
        }
        status => bail!("unexpected status code {status} for {url}"),
    };

    let progress = progress_bar(options.show_progress, total, existing, destination)?;
    let written = write_body(&mut response, &part, append, progress.as_ref())?;
    if let Some(progress) = progress {
        progress.finish_and_clear();
    }

    let size = existing + written;
    if let Some(total) = total {
        if size != total {
            bail!(
                "size mismatch: expected {total} bytes, got {size}; partial file left at {}",
                part.display()
            );
        }
    }

    if !verify_gzip(&part) {
        bail!(
            "downloaded file failed gzip verification; left at {}",
            part.display()
        );
    }

    fs::rename(&part, destination).with_context(|| {
        format!("renaming {} to {}", part.display(), destination.display())
    })?;
    println!("Download complete: {}", destination.display());
    Ok(destination.to_path_buf())
}

/// Whether `path` is a gzip file that actually decompresses.
pub fn verify_gzip(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut decoder = GzDecoder::new(file);
    let mut buffer = vec![0u8; 64 * 1024];
    let mut read_total = 0usize;

    while read_total < VERIFY_BYTES {
        match decoder.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read_total += read,
            Err(_) => return false,
        }
    }
    true
}

/// Issue the request, following redirects manually and rejecting any hop that
/// leaves the official bucket.
fn send_with_redirects(
    client: &Client,
    url: &str,
    existing: u64,
    max_redirects: usize,
) -> Result<Response> {
    let mut current = url.to_string();

    for _ in 0..=max_redirects {
        let mut request = client.get(&current);
        if existing > 0 {
            request = request.header(RANGE, format!("bytes={existing}-"));
        }
        let response = request.send().context("requesting the NLLB archive")?;

        if !is_redirect(response.status()) {
            return Ok(response);
        }

        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        if location.is_empty() {
            bail!("redirect without a Location header");
        }

        current = resolve_redirect(&current, &location)?;
        if !is_official_host(&current) {
            bail!("refusing redirect to non-official host: {current}");
        }
    }

    bail!("too many redirects while fetching {url}")
}

fn is_redirect(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    )
}

/// Resolve a `Location` value against the URL it was served from.
fn resolve_redirect(current: &str, location: &str) -> Result<String> {
    let base = Url::parse(current).with_context(|| format!("parsing {current}"))?;
    let resolved = base
        .join(location)
        .with_context(|| format!("resolving redirect to {location}"))?;
    Ok(resolved.to_string())
}

/// Total size of a `206 Partial Content` response, from `Content-Range` if the
/// server sent one, otherwise from what is left plus what we already have.
fn range_total(headers: &HeaderMap, existing: u64) -> Option<u64> {
    let content_range = headers
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok());
    if let Some(range) = content_range {
        if let Some((_, total)) = range.rsplit_once('/') {
            if let Ok(total) = total.trim().parse::<u64>() {
                return Some(total);
            }
        }
    }
    content_length(headers).map(|length| existing + length)
}

fn content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
}

fn progress_bar(
    show: bool,
    total: Option<u64>,
    existing: u64,
    destination: &Path,
) -> Result<Option<ProgressBar>> {
    if !show {
        return Ok(None);
    }
    let progress = ProgressBar::new(total.unwrap_or(0));
    progress.set_style(
        ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes}")
            .context("progress template")?
            .progress_chars("=>-"),
    );
    progress.set_message(
        destination
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Downloading".to_string()),
    );
    progress.set_position(existing);
    Ok(Some(progress))
}

/// Stream the response body into the part file, returning the bytes written.
fn write_body(
    response: &mut Response,
    part: &Path,
    append: bool,
    progress: Option<&ProgressBar>,
) -> Result<u64> {
    let mut file = if append {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(part)
            .with_context(|| format!("opening {}", part.display()))?
    } else {
        File::create(part).with_context(|| format!("creating {}", part.display()))?
    };

    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut written = 0u64;
    loop {
        let read = response.read(&mut buffer).context("reading response body")?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .with_context(|| format!("writing {}", part.display()))?;
        written += read as u64;
        if let Some(progress) = progress {
            progress.inc(read as u64);
        }
    }

    file.flush()?;
    file.sync_all()?;
    Ok(written)
}

fn part_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(PART_SUFFIX);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use reqwest::header::HeaderValue;

    fn gzip_bytes(text: &str) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(text.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }

    fn headers(pairs: &[(reqwest::header::HeaderName, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(name.clone(), HeaderValue::from_str(value).unwrap());
        }
        headers
    }

    #[test]
    fn valid_and_invalid_archives_are_told_apart() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.gz");
        fs::write(&good, gzip_bytes("hello\tsalaan\n")).unwrap();
        let bad = dir.path().join("bad.gz");
        fs::write(&bad, b"this is not gzip").unwrap();

        assert!(verify_gzip(&good));
        assert!(!verify_gzip(&bad));
        assert!(!verify_gzip(&dir.path().join("missing.gz")));
    }

    #[test]
    fn truncated_archives_fail_verification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cut.gz");
        let bytes = gzip_bytes(&"waa qoraal dheer\n".repeat(200));
        fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        assert!(!verify_gzip(&path));
    }

    #[test]
    fn total_size_comes_from_content_range() {
        let headers = headers(&[
            (CONTENT_RANGE, "bytes 10-99/100"),
            (CONTENT_LENGTH, "90"),
        ]);
        assert_eq!(range_total(&headers, 10), Some(100));
    }

    #[test]
    fn total_size_falls_back_to_what_is_left() {
        let headers = headers(&[(CONTENT_LENGTH, "90")]);
        assert_eq!(range_total(&headers, 10), Some(100));
        assert_eq!(range_total(&HeaderMap::new(), 10), None);
    }

    #[test]
    fn total_size_is_read_after_the_content_range_slash() {
        let headers = headers(&[(CONTENT_RANGE, "bytes */100")]);
        assert_eq!(range_total(&headers, 0), Some(100));
    }

    #[test]
    fn relative_redirects_resolve_against_the_current_url() {
        assert_eq!(
            resolve_redirect(
                "https://storage.googleapis.com/allennlp-data-bucket/nllb/x.gz",
                "/other/y.gz"
            )
            .unwrap(),
            "https://storage.googleapis.com/other/y.gz"
        );
    }

    #[test]
    fn absolute_redirects_replace_the_url() {
        let resolved =
            resolve_redirect("https://storage.googleapis.com/x.gz", "https://evil.example/y.gz")
                .unwrap();
        assert_eq!(resolved, "https://evil.example/y.gz");
        // The caller rejects it; resolution itself stays mechanical.
        assert!(!is_official_host(&resolved));
    }

    #[test]
    fn unofficial_source_urls_never_reach_the_network() {
        let dir = tempfile::tempdir().unwrap();
        let error = download_archive(
            "https://evil.example/x.gz",
            &dir.path().join("out.gz"),
            &DownloadOptions {
                show_progress: false,
                ..DownloadOptions::default()
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("non-official host"));
    }

    #[test]
    fn an_existing_valid_archive_is_reused() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("eng_Latn-som_Latn.gz");
        fs::write(&destination, gzip_bytes("hello\tsalaan\n")).unwrap();

        // Would fail on any network access; a valid file must short-circuit.
        let path = download_archive(
            &crate::nllb::config::official_url(),
            &destination,
            &DownloadOptions {
                show_progress: false,
                ..DownloadOptions::default()
            },
        )
        .unwrap();
        assert_eq!(path, destination);
    }

    #[test]
    fn part_files_sit_next_to_their_target() {
        assert_eq!(
            part_path(Path::new("out/nllb/eng_Latn-som_Latn.gz")),
            Path::new("out/nllb/eng_Latn-som_Latn.gz.part")
        );
    }
}
