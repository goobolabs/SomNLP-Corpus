//! Source-aware P0 normalization for the deep-clean stage.

pub fn normalize_source_text(source: &str, text: &str, unescape_madlad: bool, strip_opus_html: bool) -> String {
    let mut out = text.to_string();
    if unescape_madlad && source == "madlad" {
        out = unescape_literal_escapes(&out);
    }
    if strip_opus_html && source == "opus" {
        out = strip_opus_html_fragments(&out);
    }
    out
}

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

pub fn strip_opus_html_fragments(text: &str) -> String {
    let mut current = text.trim().to_string();
    loop {
        let lower = current.to_lowercase();
        if lower.ends_with("\\/p>") || lower.ends_with("\\/div>") || lower.ends_with("</p>") {
            if let Some(idx) = current.rfind('<') {
                current = current[..idx].trim().to_string();
                continue;
            }
        }
        if let Some(idx) = current.rfind('<') {
            let tail = &current[idx..];
            if tail.contains('>') && tail.len() < 48 {
                current = current[..idx].trim().to_string();
                continue;
            }
        }
        break;
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn madlad_unescape() {
        assert_eq!(
            normalize_source_text("madlad", r"a\nb", true, false),
            "a\nb"
        );
    }
}
