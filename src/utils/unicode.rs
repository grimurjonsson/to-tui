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

pub fn next_char_boundary(s: &str, byte_index: usize) -> usize {
    if byte_index >= s.len() {
        return s.len();
    }
    s.char_indices()
        .find(|(i, _)| *i > byte_index)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

pub fn first_char_as_str(s: &str) -> &str {
    if s.is_empty() {
        return "";
    }
    let end = s.char_indices().nth(1).map(|(i, _)| i).unwrap_or(s.len());
    &s[..end]
}

pub fn after_first_char(s: &str) -> &str {
    if s.is_empty() {
        return "";
    }
    let start = s.char_indices().nth(1).map(|(i, _)| i).unwrap_or(s.len());
    &s[start..]
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn prev_word_boundary(s: &str, byte_index: usize) -> usize {
    if byte_index == 0 {
        return 0;
    }

    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let char_pos = chars
        .iter()
        .rposition(|(i, _)| *i < byte_index)
        .unwrap_or(0);

    let mut pos = char_pos;
    while pos > 0 && !is_word_char(chars[pos].1) {
        pos -= 1;
    }
    while pos > 0 && is_word_char(chars[pos - 1].1) {
        pos -= 1;
    }

    chars.get(pos).map(|(i, _)| *i).unwrap_or(0)
}

pub fn next_word_boundary(s: &str, byte_index: usize) -> usize {
    if byte_index >= s.len() {
        return s.len();
    }

    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let char_pos = chars
        .iter()
        .position(|(i, _)| *i >= byte_index)
        .unwrap_or(chars.len());

    let mut pos = char_pos;
    while pos < chars.len() && is_word_char(chars[pos].1) {
        pos += 1;
    }
    while pos < chars.len() && !is_word_char(chars[pos].1) {
        pos += 1;
    }

    chars.get(pos).map(|(i, _)| *i).unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prev_char_boundary() {
        let s = "aöb";
        assert_eq!(prev_char_boundary(s, 0), 0);
        assert_eq!(prev_char_boundary(s, 1), 0);
        assert_eq!(prev_char_boundary(s, 3), 1);
        assert_eq!(prev_char_boundary(s, 4), 3);
    }

    #[test]
    fn test_next_char_boundary() {
        let s = "aöb";
        assert_eq!(next_char_boundary(s, 0), 1);
        assert_eq!(next_char_boundary(s, 1), 3);
        assert_eq!(next_char_boundary(s, 3), 4);
        assert_eq!(next_char_boundary(s, 4), 4);
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
    fn test_emoji() {
        let s = "👋🌍";
        assert_eq!(first_char_as_str(s), "👋");
        assert_eq!(after_first_char(s), "🌍");
    }

    #[test]
    fn test_prev_word_boundary() {
        let s = "hello world test";
        assert_eq!(prev_word_boundary(s, 16), 12);
        assert_eq!(prev_word_boundary(s, 12), 6);
        assert_eq!(prev_word_boundary(s, 6), 0);
        assert_eq!(prev_word_boundary(s, 0), 0);
        assert_eq!(prev_word_boundary(s, 3), 0);
    }

    #[test]
    fn test_next_word_boundary() {
        let s = "hello world test";
        assert_eq!(next_word_boundary(s, 0), 6);
        assert_eq!(next_word_boundary(s, 6), 12);
        assert_eq!(next_word_boundary(s, 12), 16);
        assert_eq!(next_word_boundary(s, 16), 16);
        assert_eq!(next_word_boundary(s, 3), 6);
    }

    #[test]
    fn test_word_boundary_with_punctuation() {
        let s = "hello, world!";
        assert_eq!(next_word_boundary(s, 0), 7);
        assert_eq!(prev_word_boundary(s, 13), 7);
    }
}
