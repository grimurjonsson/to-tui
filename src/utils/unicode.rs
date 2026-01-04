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
}
