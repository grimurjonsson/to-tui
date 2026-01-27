use crate::config::Config;
use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub question: Color,
    pub exclamation: Color,
    pub in_progress: Color,
    pub cancelled: Color,
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,
    pub priority_p0: Color,
    pub priority_p1: Color,
    pub priority_p2: Color,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            background: Color::Reset,
            foreground: Color::White,
            question: Color::Yellow,
            exclamation: Color::Red,
            in_progress: Color::Cyan,
            cancelled: Color::DarkGray,
            status_bar_bg: Color::Rgb(40, 40, 40),
            status_bar_fg: Color::White,
            priority_p0: Color::Rgb(255, 100, 100), // Red for critical
            priority_p1: Color::Rgb(255, 200, 100), // Yellow/orange for high
            priority_p2: Color::Rgb(100, 150, 255), // Blue for medium
        }
    }

    pub fn dark() -> Self {
        Self {
            background: Color::Black,
            foreground: Color::White,
            question: Color::Yellow,
            exclamation: Color::Red,
            in_progress: Color::Cyan,
            cancelled: Color::DarkGray,
            status_bar_bg: Color::Rgb(40, 40, 40),
            status_bar_fg: Color::White,
            priority_p0: Color::Rgb(255, 100, 100),
            priority_p1: Color::Rgb(255, 200, 100),
            priority_p2: Color::Rgb(100, 150, 255),
        }
    }

    pub fn light() -> Self {
        Self {
            background: Color::White,
            foreground: Color::Black,
            question: Color::Yellow,
            exclamation: Color::Red,
            in_progress: Color::Blue,
            cancelled: Color::Gray,
            status_bar_bg: Color::LightBlue,
            status_bar_fg: Color::Black,
            priority_p0: Color::Rgb(200, 50, 50),   // Darker red for light theme
            priority_p1: Color::Rgb(180, 130, 0),   // Darker yellow/brown for light theme
            priority_p2: Color::Rgb(50, 100, 200),  // Darker blue for light theme
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
