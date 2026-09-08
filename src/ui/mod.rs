pub mod components;
pub mod theme;

use crate::app::{AppState, event::handle_key_event, event::handle_mouse_event};
use crate::storage::UiCache;
use crate::utils::cursor::set_mouse_cursor_default;
use anyhow::Result;
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEventKind,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend, layout::Position, style::Modifier};
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

    // Create single-threaded runtime for the UI event loop
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let result = rt.block_on(run_app(&mut terminal, &mut state, plugin_rx));
    terminal.show_cursor()?;

    result?;
    Ok(state)
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    mut plugin_rx: mpsc::UnboundedReceiver<()>,
) -> Result<()> {
    let observer = crate::storage::database::get_connection()?;
    let mut data_version: i64 = observer.query_row("PRAGMA data_version", [], |row| row.get(0))?;
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

            // Capture screen buffer content for mouse text selection
            {
                let buf = f.buffer_mut();
                let area = buf.area;
                state.screen_cells.clear();
                for y in 0..area.height {
                    let mut row_cells = Vec::with_capacity(area.width as usize);
                    for x in 0..area.width {
                        if let Some(cell) = buf.cell(Position::new(x, y)) {
                            let sym = cell.symbol();
                            if sym.is_empty() {
                                row_cells.push(" ".to_string());
                            } else {
                                row_cells.push(sym.to_string());
                            }
                        } else {
                            row_cells.push(" ".to_string());
                        }
                    }
                    state.screen_cells.push(row_cells);
                }
            }

            // Apply selection highlight (reversed colors) for mouse text selection
            let selection = state.normalized_selection();
            if let Some(((sr, sc), (er, ec))) = selection {
                let buf = f.buffer_mut();
                let max_row = buf.area.height as usize;
                let max_col = buf.area.width as usize;
                for y in sr..=er.min(max_row.saturating_sub(1)) {
                    let cs = if y == sr { sc } else { 0 };
                    let ce = if y == er {
                        (ec + 1).min(max_col)
                    } else {
                        max_col
                    };
                    for x in cs..ce {
                        if let Some(cell) = buf.cell_mut(Position::new(x as u16, y as u16)) {
                            let s = cell.style().add_modifier(Modifier::REVERSED);
                            cell.set_style(s);
                        }
                    }
                }
            }
        })?;

        // Wait for ANY event source - immediate wakeup when any fires
        tokio::select! {
            biased;  // Check in priority order

            // Terminal events (keyboard, mouse)
            maybe_event = reader.next() => {
                if let Some(Ok(event)) = maybe_event {
                    match event {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            tracing::trace!(
                                "key press: code={:?} modifiers={:?}",
                                key.code,
                                key.modifiers
                            );
                            // Dismiss plugin error popup on any key press
                            if state.show_plugin_error_popup {
                                state.dismiss_plugin_error_popup();
                            } else if let Err(error) = handle_key_event(key, state) {
                                state.set_status_message(format!("{error} F5 saves a recovery copy and reloads."));
                            }
                        }
                        Event::Mouse(mouse) => {
                            if let Err(error) = handle_mouse_event(mouse, state) {
                                state.set_status_message(format!("{error} F5 saves a recovery copy and reloads."));
                            }
                        }
                        Event::Resize(_, _) => {
                            state.clear_mouse_selection();
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

            // Periodic tick for animations (spinner, status messages)
            _ = tick_interval.tick() => {
                // Don't log ticks - too noisy
                let version: i64 = observer.query_row("PRAGMA data_version", [], |row| row.get(0))?;
                if version != data_version && !state.unsaved_changes && state.mode == crate::app::mode::Mode::Navigate {
                    match state.reload_from_database() {
                        Ok(()) => data_version = version,
                        Err(error) => state.set_status_message(format!("Refresh failed: {error}")),
                    }
                }
                state.tick_spinner();
                state.check_midnight_rollover();
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
