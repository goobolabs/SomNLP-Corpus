//! Source-aware text normalization at export time (P0 fixes).

/// Unescape JSON-style literal escapes stored as two-character sequences in text.
pub fn unescape_literal_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('/') => out.push('/'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Strip trailing escaped or literal HTML tag fragments common in OPUS exports.
pub fn strip_opus_html_fragments(text: &str) -> String {
    let trimmed = text.trim();
    let mut current = trimmed.to_string();
    loop {
        let lower = current.to_lowercase();
        let stripped = if lower.ends_with("\\/p>") || lower.ends_with("\\/div>") {
            current
                .trim_end_matches(|c: char| c == '>' || c == '/' || c == '\\' || c.is_alphabetic())
                .trim()
                .to_string()
        } else if let Some(idx) = current.rfind('<') {
            let tail = &current[idx..];
            if tail.contains('>') && tail.len() < 32 {
                current[..idx].trim().to_string()
            } else {
                break;
            }
        } else {
            break;
        };
        if stripped == current {
            break;
        }
        current = stripped;
    }
    current
}

/// Apply export-time normalization for a known source key.
pub fn normalize_export_text(source: &str, text: &str) -> String {
    let mut out = text.to_string();
    if source == "madlad" {
        out = unescape_literal_escapes(&out);
    }
    if source == "opus" {
        out = strip_opus_html_fragments(&out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescapes_madlad_newlines() {
        assert_eq!(unescape_literal_escapes(r"line1\nline2"), "line1\nline2");
    }

    #[test]
    fn strips_opus_trailing_tag() {
        assert_eq!(
            strip_opus_html_fragments("Wax wanaagsan <\\/p>"),
            "Wax wanaagsan"
        );
    }
}
