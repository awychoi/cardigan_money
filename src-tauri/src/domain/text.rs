//! Untrusted text handling for provider/CSV/domain strings.

use unicode_normalization::UnicodeNormalization;

pub const MAX_MERCHANT_CHARS: usize = 200;
pub const MAX_DESCRIPTION_CHARS: usize = 500;
pub const MAX_CATEGORY_CHARS: usize = 80;
pub const MAX_NOTES_CHARS: usize = 2000;
pub const MAX_SEARCH_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextError {
    Empty,
    TooLong,
    ContainsNul,
    ControlCharacters,
}

fn is_forbidden(c: char) -> bool {
    // C0/C1 controls and bidi formatting characters that can spoof display.
    c.is_control()
        || matches!(
            c,
            '\u{202A}'..='\u{202E}'
                | '\u{2066}'..='\u{2069}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{061C}'
        )
}

/// Validate and normalize untrusted text for storage/display. Newlines are
/// allowed only for notes. Markup is preserved as text (never interpreted).
pub fn clean_text(
    input: &str,
    max_chars: usize,
    allow_newlines: bool,
) -> Result<String, TextError> {
    if input.contains('\u{0}') {
        return Err(TextError::ContainsNul);
    }
    let mut out = String::with_capacity(input.len().min(max_chars.saturating_mul(4)));
    for c in input.chars() {
        if c == '\n' || c == '\r' || c == '\t' {
            if !allow_newlines && c != '\t' {
                // Collapse single-line fields instead of failing on stray CR/LF.
                out.push(' ');
            } else if allow_newlines {
                out.push(if c == '\r' { '\n' } else { c });
            } else {
                out.push(' ');
            }
            continue;
        }
        if is_forbidden(c) {
            return Err(TextError::ControlCharacters);
        }
        out.push(c);
    }
    let out = out.trim();
    if out.is_empty() {
        return Err(TextError::Empty);
    }
    if out.chars().count() > max_chars {
        return Err(TextError::TooLong);
    }
    Ok(out.to_string())
}

/// Best-effort normalization used for matching only; display keeps the source
/// label unless an alias or rule overrides it.
pub fn normalize_merchant(input: &str) -> Result<String, TextError> {
    let cleaned = clean_text(input, MAX_DESCRIPTION_CHARS, false)?;
    let normalized: String = cleaned.nfkc().collect::<String>().to_lowercase();
    let collapsed = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed: String = collapsed.chars().take(MAX_MERCHANT_CHARS).collect();
    if trimmed.is_empty() {
        return Err(TextError::Empty);
    }
    Ok(trimmed)
}

/// Remove control and bidi characters for display while preserving length
/// within the cap. Used when rendering third-party labels.
pub fn display_safe(input: &str, max_chars: usize) -> String {
    input
        .chars()
        .filter(|c| !is_forbidden(*c))
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Escape LIKE metacharacters for a parameterized `LIKE ? ESCAPE '\'` query.
pub fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nul_controls_and_bidi() {
        assert_eq!(clean_text("hello\u{0}world", 50, false), Err(TextError::ContainsNul));
        assert_eq!(clean_text("hello\u{202E}world", 50, false), Err(TextError::ControlCharacters));
        assert_eq!(clean_text("hello\u{0007}world", 50, false), Err(TextError::ControlCharacters));
    }

    #[test]
    fn caps_length_and_collapses_single_line_fields() {
        let long = "a".repeat(201);
        assert_eq!(clean_text(&long, 200, false), Err(TextError::TooLong));
        assert_eq!(clean_text("a\nb", 10, false).unwrap(), "a b");
        assert_eq!(clean_text("  trimmed  ", 50, false).unwrap(), "trimmed");
        assert_eq!(clean_text("", 50, false), Err(TextError::Empty));
    }

    #[test]
    fn notes_allow_newlines_but_not_bidi() {
        assert_eq!(clean_text("line1\nline2", MAX_NOTES_CHARS, true).unwrap(), "line1\nline2");
        assert!(clean_text("line1\u{2066}line2", MAX_NOTES_CHARS, true).is_err());
    }

    #[test]
    fn merchant_normalization_is_case_and_space_insensitive() {
        assert_eq!(normalize_merchant("  ACME   Store  ").unwrap(), "acme store");
        assert_eq!(normalize_merchant("Café").unwrap(), "café");
        // Full-width characters are NFKC-normalized for matching.
        assert_eq!(normalize_merchant("ＡＣＭＥ").unwrap(), "acme");
    }

    #[test]
    fn display_safe_strips_controls_and_keeps_text() {
        assert_eq!(display_safe("A\u{202E}B\u{0007}C", 50), "ABC");
        assert_eq!(display_safe("<b>not html</b>", 50), "<b>not html</b>");
    }

    #[test]
    fn like_escaping() {
        assert_eq!(escape_like("100%_off\\deal"), "100\\%\\_off\\\\deal");
    }
}
