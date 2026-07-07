//! URL and email masking for deep clean.

use std::sync::OnceLock;

use regex::Regex;

pub const URL_SENTINEL: &str = "⟨url⟩";
pub const EMAIL_SENTINEL: &str = "⟨email⟩";

fn url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?:https?://|www\.)[^\s<>]+|\b[a-z0-9][a-z0-9.-]+\.(?:com|org|net|so|edu|gov|info|io|co)(?:/[^\s]*)?",
        )
        .unwrap()
    })
}

fn email_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b").unwrap())
}

pub fn mask_contacts(text: &str, mask_urls: bool, mask_emails: bool) -> String {
    let mut out = text.to_string();
    if mask_emails {
        out = email_re().replace_all(&out, EMAIL_SENTINEL).into_owned();
    }
    if mask_urls {
        out = url_re().replace_all(&out, URL_SENTINEL).into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_url_and_email() {
        let out = mask_contacts("visit https://example.com or mail a@b.com", true, true);
        assert!(out.contains(URL_SENTINEL));
        assert!(out.contains(EMAIL_SENTINEL));
    }
}
