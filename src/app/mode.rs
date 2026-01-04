use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Navigate, // Default: browse, mark, move, delete
    Edit,   // Text input for new/editing items
    Visual, // Selection mode (vim-like)
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Navigate => write!(f, "NAVIGATE"),
            Mode::Edit => write!(f, "INSERT"),
            Mode::Visual => write!(f, "VISUAL"),
        }
    }
}
