use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Navigate,
    Edit,
    Visual,
    ConfirmDelete,
    Plugin,
    Rollover,
    UpgradePrompt,
    ProjectSelect,
    MoveToProject,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Navigate => write!(f, "NAVIGATE"),
            Mode::Edit => write!(f, "INSERT"),
            Mode::Visual => write!(f, "VISUAL"),
            Mode::ConfirmDelete => write!(f, "CONFIRM"),
            Mode::Plugin => write!(f, "PLUGIN"),
            Mode::Rollover => write!(f, "ROLLOVER"),
            Mode::UpgradePrompt => write!(f, "UPGRADE"),
            Mode::ProjectSelect => write!(f, "PROJECT"),
            Mode::MoveToProject => write!(f, "MOVE"),
        }
    }
}
