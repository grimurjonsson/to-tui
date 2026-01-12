use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoState {
    Empty,       // [ ]
    Checked,     // [x]
    Question,    // [?]
    Exclamation, // [!]
    InProgress,  // [*]
}

impl TodoState {
    pub fn to_char(self) -> char {
        match self {
            Self::Empty => ' ',
            Self::Checked => 'x',
            Self::Question => '?',
            Self::Exclamation => '!',
            Self::InProgress => '*',
        }
    }

    pub fn from_char(c: char) -> Option<Self> {
        match c {
            ' ' => Some(Self::Empty),
            'x' | 'X' => Some(Self::Checked),
            '?' => Some(Self::Question),
            '!' => Some(Self::Exclamation),
            '*' => Some(Self::InProgress),
            _ => None,
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            Self::Empty => Self::InProgress,
            Self::InProgress => Self::Checked,
            Self::Checked => Self::Question,
            Self::Question => Self::Exclamation,
            Self::Exclamation => Self::Empty,
        }
    }

    pub fn toggle(&self) -> Self {
        match self {
            Self::Checked => Self::Empty,
            _ => Self::Checked,
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Checked)
    }

    /// Parse a state from a string representation.
    /// Accepts: " " or "" for Empty, "x"/"X" for Checked, "?" for Question, "!" for Exclamation, "*" for InProgress
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            " " | "" => Some(Self::Empty),
            "x" | "X" => Some(Self::Checked),
            "?" => Some(Self::Question),
            "!" => Some(Self::Exclamation),
            "*" => Some(Self::InProgress),
            _ => None,
        }
    }

}

impl fmt::Display for TodoState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.to_char())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_char() {
        assert_eq!(TodoState::Empty.to_char(), ' ');
        assert_eq!(TodoState::Checked.to_char(), 'x');
        assert_eq!(TodoState::Question.to_char(), '?');
        assert_eq!(TodoState::Exclamation.to_char(), '!');
        assert_eq!(TodoState::InProgress.to_char(), '*');
    }

    #[test]
    fn test_from_char() {
        assert_eq!(TodoState::from_char(' '), Some(TodoState::Empty));
        assert_eq!(TodoState::from_char('x'), Some(TodoState::Checked));
        assert_eq!(TodoState::from_char('X'), Some(TodoState::Checked));
        assert_eq!(TodoState::from_char('?'), Some(TodoState::Question));
        assert_eq!(TodoState::from_char('!'), Some(TodoState::Exclamation));
        assert_eq!(TodoState::from_char('*'), Some(TodoState::InProgress));
        assert_eq!(TodoState::from_char('z'), None);
    }

    #[test]
    fn test_cycle() {
        assert_eq!(TodoState::Empty.cycle(), TodoState::InProgress);
        assert_eq!(TodoState::InProgress.cycle(), TodoState::Checked);
        assert_eq!(TodoState::Checked.cycle(), TodoState::Question);
        assert_eq!(TodoState::Question.cycle(), TodoState::Exclamation);
        assert_eq!(TodoState::Exclamation.cycle(), TodoState::Empty);
    }

    #[test]
    fn test_is_complete() {
        assert!(!TodoState::Empty.is_complete());
        assert!(TodoState::Checked.is_complete());
        assert!(!TodoState::Question.is_complete());
        assert!(!TodoState::Exclamation.is_complete());
        assert!(!TodoState::InProgress.is_complete());
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", TodoState::Empty), "[ ]");
        assert_eq!(format!("{}", TodoState::Checked), "[x]");
        assert_eq!(format!("{}", TodoState::Question), "[?]");
        assert_eq!(format!("{}", TodoState::Exclamation), "[!]");
        assert_eq!(format!("{}", TodoState::InProgress), "[*]");
    }
}
