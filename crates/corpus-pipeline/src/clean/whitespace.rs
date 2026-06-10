//! Whitespace normalization that preserves paragraph breaks. Per line, collapse
//! internal whitespace and trim; collapse multiple blank lines to a single
//! paragraph break. See docs/CLEANING_PLAN.md (Clean stage).

fn collapse_spaces(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut last_space = false;
    for c in line.chars() {
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out.trim().to_string()
}

/// Collapse intra-line whitespace and cap consecutive blank lines at one,
/// preserving paragraph boundaries. Leading and trailing blank lines are dropped.
pub fn normalize_whitespace(text: &str) -> String {
    let mut out_lines: Vec<String> = Vec::new();
    let mut pending_blank = false;
    for line in text.split('\n') {
        let collapsed = collapse_spaces(line);
        if collapsed.is_empty() {
            if !out_lines.is_empty() {
                pending_blank = true;
            }
        } else {
            if pending_blank {
                out_lines.push(String::new());
                pending_blank = false;
            }
            out_lines.push(collapsed);
        }
    }
    out_lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_internal_runs() {
        assert_eq!(normalize_whitespace("a   b\t\tc"), "a b c");
    }

    #[test]
    fn preserves_single_paragraph_break() {
        assert_eq!(normalize_whitespace("para one\n\npara two"), "para one\n\npara two");
    }

    #[test]
    fn collapses_multiple_blank_lines() {
        assert_eq!(
            normalize_whitespace("a\n\n\n\nb"),
            "a\n\nb"
        );
    }

    #[test]
    fn trims_leading_and_trailing_blanks() {
        assert_eq!(normalize_whitespace("\n\n  hi  \n\n"), "hi");
    }
}
