//! HTML two-tier policy for deep clean.

use std::sync::OnceLock;

use common::types::QualityFlag;
use regex::Regex;

fn script_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?is)<\?(php)?.*?\?>|<script\b.*?</script>|<style\b.*?</style>|<mutation\b.*?>|<char\b.*?>")
            .unwrap()
    })
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)</?[a-zA-Z][a-zA-Z0-9]*(\s[^>]*)?>").unwrap())
}

pub fn has_reject_html(text: &str) -> bool {
    script_re().is_match(text)
}

pub fn strip_benign_tags(text: &str) -> String {
    tag_re().replace_all(text, "").into_owned()
}

pub fn apply_html_policy(
    text: &str,
    reject_script: bool,
    strip_benign: bool,
) -> Result<String, QualityFlag> {
    if reject_script && has_reject_html(text) {
        return Err(QualityFlag::HtmlRemnant);
    }
    let stripped = if strip_benign {
        strip_benign_tags(text)
    } else {
        text.to_string()
    };
    Ok(stripped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_script() {
        assert!(has_reject_html("hello <?php echo x; ?>"));
    }

    #[test]
    fn strips_inline_tags() {
        assert_eq!(strip_benign_tags("a <b>bold</b> c"), "a bold c");
    }
}
