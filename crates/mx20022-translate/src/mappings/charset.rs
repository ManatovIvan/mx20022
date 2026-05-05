//! SWIFT FIN character-set conversion utilities.
//!
//! The SWIFT FIN character set is a restricted subset of ASCII.  ISO 20022
//! messages use UTF-8 and may contain characters that cannot be transmitted
//! in a SWIFT FIN message without replacement.

/// The SWIFT FIN character set: letters, digits and the punctuation symbols
/// that are explicitly permitted in MT message text blocks.
///
/// Allowed: `A-Z`, `a-z`, `0-9`, `/ - ? : ( ) . , ' + { } CR LF Space`
///
/// # Examples
///
/// ```rust
/// use mx20022_translate::mappings::charset::is_swift_safe;
/// assert!(is_swift_safe('A'));
/// assert!(is_swift_safe('/'));
/// assert!(!is_swift_safe('€'));
/// ```
#[must_use]
pub fn is_swift_safe(c: char) -> bool {
    matches!(
        c,
        'A'..='Z'
            | 'a'..='z'
            | '0'..='9'
            | ' '
            | '/'
            | '-'
            | '?'
            | ':'
            | '('
            | ')'
            | '.'
            | ','
            | '\''
            | '+'
            | '{'
            | '}'
            | '\r'
            | '\n'
    )
}

/// Convert a UTF-8 string to the SWIFT FIN character set.
///
/// Characters outside the SWIFT charset are replaced with a close ASCII
/// approximation when one exists, or with a space otherwise.
///
/// Returns `(converted_string, had_replacements)`.  The boolean is `true`
/// when at least one character was replaced.
///
/// # Examples
///
/// ```rust
/// use mx20022_translate::mappings::charset::to_swift_charset;
/// let (s, replaced) = to_swift_charset("Müller");
/// assert_eq!(s, "Muller");
/// assert!(replaced);
/// ```
pub fn to_swift_charset(s: &str) -> (String, bool) {
    let mut out = String::with_capacity(s.len());
    let mut had_replacements = false;

    for c in s.chars() {
        if is_swift_safe(c) {
            out.push(c);
        } else {
            had_replacements = true;
            let replacement = approximate(c);
            out.push_str(replacement);
        }
    }

    (out, had_replacements)
}

/// Return an ASCII approximation for non-SWIFT characters.
///
/// Covers the most common European diacritics and Unicode punctuation.
fn approximate(c: char) -> &'static str {
    match c {
        // Latin extended — uppercase
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => "A",
        'Æ' => "AE",
        'Ç' => "C",
        'È' | 'É' | 'Ê' | 'Ë' => "E",
        'Ì' | 'Í' | 'Î' | 'Ï' => "I",
        'Ð' => "D",
        'Ñ' => "N",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => "O",
        'Ù' | 'Ú' | 'Û' | 'Ü' => "U",
        'Ý' => "Y",
        'Þ' => "TH",
        'ß' => "ss",
        // Latin extended — lowercase
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => "a",
        'æ' => "ae",
        'ç' => "c",
        'è' | 'é' | 'ê' | 'ë' => "e",
        'ì' | 'í' | 'î' | 'ï' => "i",
        'ð' => "d",
        'ñ' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => "o",
        'ù' | 'ú' | 'û' | 'ü' => "u",
        'ý' | 'ÿ' => "y",
        'þ' => "th",
        // Currency and common symbols
        '€' => "EUR",
        '£' => "GBP",
        '¥' => "JPY",
        // Typographic punctuation — replace with safe equivalents
        // Single/double quotation marks → apostrophe
        '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}' => "'",
        '\u{2013}' | '\u{2014}' => "-", // en-dash / em-dash
        '\u{2026}' => "...",            // horizontal ellipsis
        // Copyright/trademark symbols, tabs and everything else → space
        _ => " ",
    }
}

// ---------------------------------------------------------------------------
// Line wrapping
// ---------------------------------------------------------------------------

/// Result of [`wrap_lines`] when the wrapped output exceeded `max_lines`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapError {
    /// The first `max_lines` lines that were produced — what a lossy
    /// caller would emit if it elected to truncate.
    pub truncated: Vec<String>,
    /// Number of characters from the input that did not fit into
    /// `truncated`.
    pub overflow_chars: usize,
}

/// Wrap `text` into lines no longer than `max_line_len` characters,
/// up to `max_lines` lines.
///
/// Wrapping happens at ASCII-whitespace boundaries when one is available
/// inside the line; words longer than `max_line_len` are hard-cut at
/// the character boundary. SWIFT MT text is ASCII (see
/// [`is_swift_safe`]) so a character is a byte and `len()` and
/// position-based slicing are safe.
///
/// Returns `Ok(lines)` when the input fits in `max_lines × max_line_len`.
/// Returns `Err(WrapError)` carrying the truncated output and the
/// number of characters that could not be placed, so the caller can
/// decide whether to warn-and-truncate or refuse.
///
/// # Examples
///
/// ```
/// use mx20022_translate::mappings::charset::wrap_lines;
///
/// // Fits.
/// let lines = wrap_lines("ACME CORPORATION INTERNATIONAL", 35, 4).unwrap();
/// assert_eq!(lines, vec!["ACME CORPORATION INTERNATIONAL"]);
///
/// // Word-wraps at the space boundary.
/// let lines = wrap_lines("ACME CORPORATION INTERNATIONAL LIMITED", 20, 4).unwrap();
/// assert_eq!(lines, vec!["ACME CORPORATION", "INTERNATIONAL", "LIMITED"]);
///
/// // Hard-cuts a too-long word.
/// let lines = wrap_lines("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", 10, 4).unwrap();
/// assert_eq!(lines, vec!["AAAAAAAAAA"; 4]);
///
/// // Overflows max_lines.
/// let err = wrap_lines("one two three four five six seven", 5, 2).unwrap_err();
/// assert_eq!(err.truncated.len(), 2);
/// assert!(err.overflow_chars > 0);
/// ```
///
/// # Panics
///
/// Panics if `max_line_len` or `max_lines` is zero.
pub fn wrap_lines(
    text: &str,
    max_line_len: usize,
    max_lines: usize,
) -> Result<Vec<String>, WrapError> {
    assert!(
        max_line_len > 0,
        "wrap_lines max_line_len must be > 0, got {max_line_len}"
    );
    assert!(
        max_lines > 0,
        "wrap_lines max_lines must be > 0, got {max_lines}"
    );

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in trimmed.split_ascii_whitespace() {
        // Hard-cut a word longer than max_line_len before considering
        // whether it fits in the current line.
        let mut word_remaining = word;
        while word_remaining.len() > max_line_len {
            // Flush current line if it has content; the long word
            // should start at column 0 for hard cuts.
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let (head, tail) = word_remaining.split_at(max_line_len);
            lines.push(head.to_string());
            word_remaining = tail;
        }

        // word_remaining now fits within max_line_len.
        let needed = if current.is_empty() {
            word_remaining.len()
        } else {
            current.len() + 1 + word_remaining.len()
        };

        if needed > max_line_len {
            lines.push(std::mem::take(&mut current));
            current.push_str(word_remaining);
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word_remaining);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.len() <= max_lines {
        Ok(lines)
    } else {
        let truncated: Vec<String> = lines.iter().take(max_lines).cloned().collect();
        let kept_chars: usize =
            truncated.iter().map(String::len).sum::<usize>() + truncated.len().saturating_sub(1); // joining spaces between lines
        let total_chars: usize = trimmed
            .split_ascii_whitespace()
            .map(str::len)
            .sum::<usize>()
            + trimmed.split_ascii_whitespace().count().saturating_sub(1);
        Err(WrapError {
            truncated,
            overflow_chars: total_chars.saturating_sub(kept_chars),
        })
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_letters_are_safe() {
        for c in 'A'..='Z' {
            assert!(is_swift_safe(c));
        }
        for c in 'a'..='z' {
            assert!(is_swift_safe(c));
        }
    }

    #[test]
    fn test_digits_are_safe() {
        for c in '0'..='9' {
            assert!(is_swift_safe(c));
        }
    }

    #[test]
    fn test_swift_punctuation_safe() {
        for c in [
            ' ', '/', '-', '?', ':', '(', ')', '.', ',', '\'', '+', '{', '}',
        ] {
            assert!(is_swift_safe(c), "expected '{c}' to be SWIFT-safe");
        }
    }

    #[test]
    fn test_non_swift_chars_not_safe() {
        assert!(!is_swift_safe('€'));
        assert!(!is_swift_safe('ü'));
        assert!(!is_swift_safe('ñ'));
    }

    #[test]
    fn test_pure_ascii_no_replacement() {
        let (s, replaced) = to_swift_charset("HELLO WORLD 123");
        assert_eq!(s, "HELLO WORLD 123");
        assert!(!replaced);
    }

    #[test]
    fn test_umlaut_replacement() {
        let (s, replaced) = to_swift_charset("Müller");
        assert_eq!(s, "Muller");
        assert!(replaced);
    }

    #[test]
    fn test_euro_sign_replacement() {
        let (s, replaced) = to_swift_charset("100€");
        assert_eq!(s, "100EUR");
        assert!(replaced);
    }

    #[test]
    fn test_empty_string() {
        let (s, replaced) = to_swift_charset("");
        assert_eq!(s, "");
        assert!(!replaced);
    }

    // -----------------------------------------------------------------------
    // wrap_lines
    // -----------------------------------------------------------------------

    #[test]
    fn test_wrap_lines_empty() {
        assert_eq!(wrap_lines("", 35, 4).unwrap(), Vec::<String>::new());
        assert_eq!(wrap_lines("   ", 35, 4).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn test_wrap_lines_short_fits_one_line() {
        assert_eq!(wrap_lines("HELLO", 35, 4).unwrap(), vec!["HELLO"]);
    }

    #[test]
    fn test_wrap_lines_exact_line_length() {
        let s = "A".repeat(35);
        assert_eq!(wrap_lines(&s, 35, 4).unwrap(), vec![s]);
    }

    #[test]
    fn test_wrap_lines_word_boundary() {
        let lines = wrap_lines("ACME CORPORATION INTERNATIONAL LIMITED", 20, 4).unwrap();
        assert_eq!(lines, vec!["ACME CORPORATION", "INTERNATIONAL", "LIMITED"]);
    }

    #[test]
    fn test_wrap_lines_hard_cut_long_word() {
        // 40-char word, max_line_len 10 → 4 lines of 10 chars each.
        let lines = wrap_lines(&"A".repeat(40), 10, 4).unwrap();
        assert_eq!(lines, vec!["A".repeat(10); 4]);
    }

    #[test]
    fn test_wrap_lines_overflow_returns_truncated_and_chars() {
        // "one two three four five six seven" — 7 words of varying sizes.
        // max_line_len 5, max_lines 2.
        let err = wrap_lines("one two three four five six seven", 5, 2).unwrap_err();
        assert_eq!(err.truncated.len(), 2);
        for line in &err.truncated {
            assert!(line.len() <= 5, "line over budget: {line:?}");
        }
        assert!(
            err.overflow_chars > 0,
            "expected overflow_chars > 0, got {}",
            err.overflow_chars
        );
    }

    #[test]
    fn test_wrap_lines_real_mt_party_name() {
        // Realistic MT103 :50K: name + 3 address lines, 35 cols max.
        let text = "JOHN JACOB JINGLEHEIMER SCHMIDT III 1234 ELM STREET APT 12 SPRINGFIELD ILLINOIS 62701 USA";
        let lines = wrap_lines(text, 35, 4).unwrap();
        assert!(lines.len() <= 4);
        for line in &lines {
            assert!(line.len() <= 35, "line {:?} exceeds 35 cols", line);
        }
    }

    #[test]
    #[should_panic(expected = "max_line_len must be > 0")]
    fn test_wrap_lines_zero_line_len_panics() {
        let _ = wrap_lines("X", 0, 4);
    }

    #[test]
    #[should_panic(expected = "max_lines must be > 0")]
    fn test_wrap_lines_zero_max_lines_panics() {
        let _ = wrap_lines("X", 35, 0);
    }
}
