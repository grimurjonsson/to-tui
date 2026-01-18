pub mod components;
pub mod theme;

use crate::app::{event::handle_key_event, event::handle_mouse_event, AppState};
use crate::storage::UiCache;
use crate::utils::paths::get_database_path;
use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Write};
use std::sync::mpsc;
use std::time::Duration;

struct TerminalGuard {
    keyboard_enhancement: bool,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        if self.keyboard_enhancement {
            let _ = execute!(stdout, PopKeyboardEnhancementFlags);
        }
        let _ = disable_raw_mode();
        let _ = execute!(stdout, DisableMouseCapture, LeaveAlternateScreen);
        let _ = stdout.flush();
    }
}

pub fn run_tui(mut state: AppState) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let supports_keyboard_enhancement = execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )
    .is_ok();

    let _guard = TerminalGuard {
        keyboard_enhancement: supports_keyboard_enhancement,
    };

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (db_tx, db_rx) = mpsc::channel();
    let _watcher = setup_database_watcher(db_tx);

    let result = run_app(&mut terminal, &mut state, db_rx);
    terminal.show_cursor()?;

    result
}

fn setup_database_watcher(tx: mpsc::Sender<()>) -> Option<RecommendedWatcher> {
    let db_path = match get_database_path() {
        Ok(path) => path,
        Err(_) => return None,
    };

    let watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res
                && event.kind.is_modify() {
                    let _ = tx.send(());
                }
        },
        Config::default(),
    );

    match watcher {
        Ok(mut w) => {
            if w.watch(&db_path, RecursiveMode::NonRecursive).is_ok() {
                Some(w)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    db_rx: mpsc::Receiver<()>,
) -> Result<()> {
    loop {
        state.clear_expired_status_message();
        state.check_plugin_result();
        state.tick_spinner();

        terminal.draw(|f| {
            components::render(f, state);
        })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    handle_key_event(key, state)?;
                }
                Event::Mouse(mouse) => {
                    handle_mouse_event(mouse, state)?;
                }
                _ => {}
            }
        }

        let mut should_reload = false;
        while db_rx.try_recv().is_ok() {
            should_reload = true;
        }
        if should_reload {
            let _ = state.reload_from_database();
        }

        if state.should_quit {
            // Save UI cache before quitting
            let cache = UiCache {
                selected_todo_id: state.get_selected_todo_id(),
            };
            let _ = cache.save(); // Ignore errors on save
            break;
        }
    }

    Ok(())
}
