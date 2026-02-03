pub mod components;
pub mod theme;

use crate::app::{event::handle_key_event, event::handle_mouse_event, AppState};
use crate::storage::UiCache;
use crate::utils::cursor::set_mouse_cursor_default;
use crate::utils::paths::get_database_path;
use anyhow::Result;
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEventKind,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Write};
use std::time::Duration;
use tokio::sync::mpsc;

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
        // Reset mouse cursor to default in case it was changed to pointer
        set_mouse_cursor_default();
        let _ = stdout.flush();
    }
}

pub fn run_tui(mut state: AppState) -> Result<AppState> {
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

    // Initialize plugin notification channel
    let plugin_rx = crate::plugin::loader::init_plugin_notifier();

    // Set up database watcher with tokio channel
    let (db_tx, db_rx) = mpsc::unbounded_channel();
    let _watcher = setup_database_watcher(db_tx);

    // Create single-threaded runtime for the UI event loop
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let result = rt.block_on(run_app(&mut terminal, &mut state, db_rx, plugin_rx));
    terminal.show_cursor()?;

    result?;
    Ok(state)
}

fn setup_database_watcher(tx: mpsc::UnboundedSender<()>) -> Option<RecommendedWatcher> {
    let db_path = match get_database_path() {
        Ok(path) => path,
        Err(_) => return None,
    };

    let watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res
                && event.kind.is_modify()
            {
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

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    mut db_rx: mpsc::UnboundedReceiver<()>,
    mut plugin_rx: mpsc::UnboundedReceiver<()>,
) -> Result<()> {
    let mut reader = EventStream::new();
    let mut tick_interval = tokio::time::interval(Duration::from_millis(100));

    loop {
        // State maintenance
        state.clear_expired_status_message();
        state.check_plugin_result();
        state.check_marketplace_fetch();
        state.check_version_update();
        state.check_download_progress();
        state.check_plugin_download_progress();

        // Poll and apply hook results
        state.apply_pending_hook_results();

        // Render
        terminal.draw(|f| {
            components::render(f, state);
        })?;

        // Wait for ANY event source - immediate wakeup when any fires
        tokio::select! {
            biased;  // Check in priority order

            // Terminal events (keyboard, mouse)
            maybe_event = reader.next() => {
                if let Some(Ok(event)) = maybe_event {
                    match event {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            // Dismiss plugin error popup on any key press
                            if state.show_plugin_error_popup {
                                state.dismiss_plugin_error_popup();
                            } else {
                                handle_key_event(key, state)?;
                            }
                        }
                        Event::Mouse(mouse) => {
                            handle_mouse_event(mouse, state)?;
                        }
                        _ => {}
                    }
                }
            }

            // Plugin signaled it has updates
            _ = plugin_rx.recv() => {
                tracing::info!("UI loop: Received plugin update notification, firing OnLoad event");
                state.fire_on_load_event();
            }

            // Database file changed externally
            _ = db_rx.recv() => {
                tracing::debug!("UI loop: Database file changed, reloading");
                let _ = state.reload_from_database();
            }

            // Periodic tick for animations (spinner, status messages)
            _ = tick_interval.tick() => {
                // Don't log ticks - too noisy
                state.tick_spinner();
            }
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
