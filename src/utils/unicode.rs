/// UTF-8 safe string operations for cursor positioning and slicing.
///
/// Rust strings are UTF-8 encoded, where characters can be 1-4 bytes.
/// These helpers ensure we always operate on character boundaries.

/// Get the byte index for a given character position.
/// Returns the byte index at the start of the nth character,
/// or the string length if pos >= character count.
pub fn char_pos_to_byte_index(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Get the byte index of the previous character boundary.
/// Returns 0 if already at the start.
pub fn prev_char_boundary(s: &str, byte_index: usize) -> usize {
    if byte_index == 0 {
        return 0;
    }
    s.char_indices()
        .rev()
        .find(|(i, _)| *i < byte_index)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Get the byte index of the next character boundary.
/// Returns string length if already at the end.
pub fn next_char_boundary(s: &str, byte_index: usize) -> usize {
    if byte_index >= s.len() {
        return s.len();
    }
    s.char_indices()
        .find(|(i, _)| *i > byte_index)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Get the first character of a string as a string slice.
/// Returns an empty string if the input is empty.
pub fn first_char_as_str(s: &str) -> &str {
    if s.is_empty() {
        return "";
    }
    let end = s.char_indices()
        .nth(1)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    &s[..end]
}

/// Get the remainder of a string after the first character.
/// Returns an empty string if the input is empty or has only one character.
pub fn after_first_char(s: &str) -> &str {
    if s.is_empty() {
        return "";
    }
    let start = s.char_indices()
        .nth(1)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    &s[start..]
}

/// Truncate a string to at most `max_chars` characters.
/// Returns the original string if it has fewer characters.
pub fn truncate_chars(s: &str, max_chars: usize) -> &str {
    let byte_index = char_pos_to_byte_index(s, max_chars);
    &s[..byte_index]
}

/// Count the number of characters in a string.
pub fn char_count(s: &str) -> usize {
    s.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_pos_to_byte_index_ascii() {
        let s = "hello";
        assert_eq!(char_pos_to_byte_index(s, 0), 0);
        assert_eq!(char_pos_to_byte_index(s, 1), 1);
        assert_eq!(char_pos_to_byte_index(s, 5), 5);
        assert_eq!(char_pos_to_byte_index(s, 10), 5); // Beyond end
    }

    #[test]
    fn test_char_pos_to_byte_index_unicode() {
        let s = "öðólæþ"; // ö=2, ð=2, ó=2, l=1, æ=2, þ=2 = 11 bytes
        assert_eq!(char_pos_to_byte_index(s, 0), 0);
        assert_eq!(char_pos_to_byte_index(s, 1), 2);  // After ö
        assert_eq!(char_pos_to_byte_index(s, 2), 4);  // After ð
        assert_eq!(char_pos_to_byte_index(s, 6), 11); // End of string
        assert_eq!(char_pos_to_byte_index(s, 10), 11); // Beyond end
    }

    #[test]
    fn test_char_pos_to_byte_index_mixed() {
        let s = "aöb"; // a=1, ö=2, b=1 = 4 bytes
        assert_eq!(char_pos_to_byte_index(s, 0), 0);
        assert_eq!(char_pos_to_byte_index(s, 1), 1);  // After a
        assert_eq!(char_pos_to_byte_index(s, 2), 3);  // After ö
        assert_eq!(char_pos_to_byte_index(s, 3), 4);  // After b
    }

    #[test]
    fn test_prev_char_boundary() {
        let s = "aöb";
        assert_eq!(prev_char_boundary(s, 0), 0);
        assert_eq!(prev_char_boundary(s, 1), 0);  // Before ö -> a's start
        assert_eq!(prev_char_boundary(s, 3), 1);  // Before b -> ö's start
        assert_eq!(prev_char_boundary(s, 4), 3);  // After b -> b's start
    }

    #[test]
    fn test_next_char_boundary() {
        let s = "aöb";
        assert_eq!(next_char_boundary(s, 0), 1);  // After a
        assert_eq!(next_char_boundary(s, 1), 3);  // After ö
        assert_eq!(next_char_boundary(s, 3), 4);  // After b
        assert_eq!(next_char_boundary(s, 4), 4);  // At end
    }

    #[test]
    fn test_first_char_as_str() {
        assert_eq!(first_char_as_str("hello"), "h");
        assert_eq!(first_char_as_str("öðólæþ"), "ö");
        assert_eq!(first_char_as_str(""), "");
        assert_eq!(first_char_as_str("a"), "a");
    }

    #[test]
    fn test_after_first_char() {
        assert_eq!(after_first_char("hello"), "ello");
        assert_eq!(after_first_char("öðólæþ"), "ðólæþ");
        assert_eq!(after_first_char(""), "");
        assert_eq!(after_first_char("a"), "");
    }

    #[test]
    fn test_truncate_chars() {
        assert_eq!(truncate_chars("hello", 3), "hel");
        assert_eq!(truncate_chars("öðólæþ", 3), "öðó");
        assert_eq!(truncate_chars("hello", 10), "hello");
        assert_eq!(truncate_chars("", 5), "");
    }

    #[test]
    fn test_char_count() {
        assert_eq!(char_count("hello"), 5);
        assert_eq!(char_count("öðólæþ"), 6);
        assert_eq!(char_count(""), 0);
        assert_eq!(char_count("aöb"), 3);
    }

    #[test]
    fn test_emoji() {
        let s = "👋🌍"; // Each emoji is 4 bytes
        assert_eq!(char_count(s), 2);
        assert_eq!(char_pos_to_byte_index(s, 1), 4);
        assert_eq!(first_char_as_str(s), "👋");
        assert_eq!(after_first_char(s), "🌍");
    }
}
