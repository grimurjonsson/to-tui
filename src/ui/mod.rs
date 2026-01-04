pub mod components;
pub mod theme;

use crate::app::{event::handle_key_event, AppState};
use crate::utils::paths::get_database_path;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Write};
use std::sync::mpsc;
use std::time::Duration;

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = disable_raw_mode();
        let _ = execute!(stdout, LeaveAlternateScreen);
        let _ = stdout.flush();
    }
}

pub fn run_tui(mut state: AppState) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let _guard = TerminalGuard;

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
            if let Ok(event) = res {
                if event.kind.is_modify() {
                    let _ = tx.send(());
                }
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
        terminal.draw(|f| {
            components::render(f, state);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key_event(key, state)?;
                }
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
            break;
        }
    }

    Ok(())
}
