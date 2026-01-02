pub mod components;
pub mod theme;

use crate::app::{event::handle_key_event, AppState};
use anyhow::Result;
use crossterm::{
    event::{
        self, Event, KeyboardEnhancementFlags, KeyEventKind, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

pub fn run_tui(mut state: AppState) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();

    // Enable enhanced keyboard support (Kitty protocol) for proper modifier detection
    // This allows Shift+Enter and other modifier combinations to work correctly
    let keyboard_enhancement_enabled = supports_keyboard_enhancement().unwrap_or(false);
    if keyboard_enhancement_enabled {
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
    }

    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    let result = run_app(&mut terminal, &mut state);

    // Cleanup terminal
    disable_raw_mode()?;
    if keyboard_enhancement_enabled {
        execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags)?;
    }
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, state: &mut AppState) -> Result<()> {
    loop {
        terminal.draw(|f| {
            components::render(f, state);
        })?;

        // Poll for events with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key_event(key, state)?;
                }
            }
        }

        if state.should_quit {
            break;
        }
    }

    Ok(())
}
