//! HTML entity decoding. Decodes all named and numeric entities exactly once.

/// Decode HTML entities (named like `&amp;` and numeric like `&#8217;`) a single
/// time. Decoding once avoids over-decoding chains such as `&amp;amp;`.
pub fn decode_entities(text: &str) -> String {
    html_escape::decode_html_entities(text).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_named_and_numeric() {
        assert_eq!(decode_entities("a &amp; b"), "a & b");
        assert_eq!(decode_entities("it&#39;s"), "it's");
        assert_eq!(decode_entities("&#8217;"), "\u{2019}");
    }

    #[test]
    fn decodes_only_once() {
        // &amp;amp; -> &amp; (not &)
        assert_eq!(decode_entities("&amp;amp;"), "&amp;");
    }
}
