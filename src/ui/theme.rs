use crate::config::Config;
use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub question: Color,
    pub exclamation: Color,
    pub in_progress: Color,
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            background: Color::Reset,
            foreground: Color::White,
            question: Color::Yellow,
            exclamation: Color::Red,
            in_progress: Color::Cyan,
            status_bar_bg: Color::DarkGray,
            status_bar_fg: Color::White,
        }
    }

    pub fn dark() -> Self {
        Self {
            background: Color::Black,
            foreground: Color::White,
            question: Color::Yellow,
            exclamation: Color::Red,
            in_progress: Color::Cyan,
            status_bar_bg: Color::DarkGray,
            status_bar_fg: Color::White,
        }
    }

    pub fn light() -> Self {
        Self {
            background: Color::White,
            foreground: Color::Black,
            question: Color::Yellow,
            exclamation: Color::Red,
            in_progress: Color::Blue,
            status_bar_bg: Color::LightBlue,
            status_bar_fg: Color::Black,
        }
    }

    pub fn from_config(config: &Config) -> Self {
        match config.theme.as_str() {
            "dark" => Self::dark(),
            "light" => Self::light(),
            _ => Self::default_theme(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_theme()
    }
}
