//! Plugins modal component for the tabbed plugin browser.
//!
//! This module renders the P-key plugins modal with:
//! - Installed tab showing loaded plugins
//! - Marketplace tab showing available plugins from registry
//! - Input view for plugin parameter entry
//! - Preview view for generated items
//! - Error view for displaying errors

use crate::app::state::{PluginsModalState, PluginsTab};
use crate::app::AppState;
use crate::plugin::marketplace::PluginEntry;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};

/// Render the plugins modal based on current state
pub fn render_plugins_modal(f: &mut Frame, state: &AppState) {
    let modal_state = match &state.plugins_modal_state {
        Some(ms) => ms,
        None => return,
    };

    match modal_state {
        PluginsModalState::Tabs {
            active_tab,
            installed_index,
            marketplace_index,
            marketplace_plugins,
            marketplace_loading,
            marketplace_error,
            marketplace_name,
        } => render_tabs_view(
            f,
            state,
            *active_tab,
            *installed_index,
            *marketplace_index,
            marketplace_plugins.as_deref(),
            *marketplace_loading,
            marketplace_error.as_deref(),
            marketplace_name,
        ),
        PluginsModalState::Details { plugin, .. } => render_details_view(f, state, plugin),
        PluginsModalState::Input {
            plugin_name,
            input_buffer,
            cursor_pos,
        } => render_input_view(f, state, plugin_name, input_buffer, *cursor_pos),
        PluginsModalState::SelectInput {
            plugin_name,
            field_name,
            options,
            selected_index,
        } => render_select_input_view(f, state, plugin_name, field_name, options, *selected_index),
        PluginsModalState::Executing { plugin_name } => render_executing_view(f, state, plugin_name),
        PluginsModalState::Preview { items } => render_preview_view(f, state, items),
        PluginsModalState::Error { message } => render_error_view(f, state, message),
    }
}

/// Render the tabbed view with Installed and Marketplace tabs
#[allow(clippy::too_many_arguments)]
fn render_tabs_view(
    f: &mut Frame,
    state: &AppState,
    active_tab: PluginsTab,
    installed_index: usize,
    marketplace_index: usize,
    marketplace_plugins: Option<&[PluginEntry]>,
    marketplace_loading: bool,
    marketplace_error: Option<&str>,
    marketplace_name: &str,
) {
    let area = centered_rect(60, 60, f.area());

    // Clear background
    f.render_widget(Clear, area);

    // Create main block
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Plugins ")
        .style(Style::default().bg(state.theme.background));

    f.render_widget(block, area);

    // Inner area for content
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Layout: tabs header + content + footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tab bar
            Constraint::Length(1), // Separator
            Constraint::Min(1),    // Content
            Constraint::Length(1), // Footer
        ])
        .split(inner);

    // Render tab bar
    let tab_titles = vec!["Installed", "Marketplace"];
    let selected_tab = match active_tab {
        PluginsTab::Installed => 0,
        PluginsTab::Marketplace => 1,
    };

    let tabs = Tabs::new(tab_titles)
        .select(selected_tab)
        .style(Style::default().fg(state.theme.foreground))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider(" | ");

    f.render_widget(tabs, chunks[0]);

    // Render separator line
    let separator = Paragraph::new(Line::from("─".repeat(chunks[1].width as usize)))
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(separator, chunks[1]);

    // Render tab content
    match active_tab {
        PluginsTab::Installed => render_installed_list(f, state, chunks[2], installed_index),
        PluginsTab::Marketplace => render_marketplace_list(
            f,
            state,
            chunks[2],
            marketplace_index,
            marketplace_plugins,
            marketplace_loading,
            marketplace_error,
            marketplace_name,
        ),
    }

    // Render footer
    let footer_text = match active_tab {
        PluginsTab::Installed => "[Tab] switch | [j/k] navigate | [Enter] invoke | [Esc] close",
        PluginsTab::Marketplace => "[Tab] switch | [j/k] navigate | [Enter] details | [Esc] close",
    };
    let footer = Paragraph::new(Line::from(Span::styled(
        footer_text,
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(footer, chunks[3]);
}

/// Render the installed plugins list
fn render_installed_list(f: &mut Frame, state: &AppState, area: Rect, selected_index: usize) {
    // Collect and sort by name for stable ordering (HashMap iteration is non-deterministic)
    let mut plugins: Vec<_> = state.plugin_loader.loaded_plugins().collect();
    plugins.sort_by(|a, b| a.name.cmp(&b.name));

    if plugins.is_empty() {
        // Check if any plugins are installed on disk but not loaded
        let has_unloaded = crate::utils::paths::get_plugins_dir()
            .ok()
            .and_then(|dir| std::fs::read_dir(dir).ok())
            .map(|entries| entries.filter_map(|e| e.ok()).any(|e| e.path().is_dir()))
            .unwrap_or(false);

        let empty_msg = if has_unloaded {
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "Plugins installed but not loaded.",
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(Span::styled(
                    "Restart totui to load newly installed plugins.",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        } else {
            Paragraph::new(Line::from(vec![
                Span::styled("No plugins installed. ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "Use Marketplace tab to browse and install.",
                    Style::default().fg(Color::Yellow),
                ),
            ]))
        }
        .wrap(Wrap { trim: true });
        f.render_widget(empty_msg, area);
        return;
    }

    let items: Vec<ListItem> = plugins
        .iter()
        .enumerate()
        .map(|(i, plugin)| {
            let is_selected = i == selected_index;

            // Status indicator
            let status = if plugin.session_disabled {
                Span::styled("[X]", Style::default().fg(Color::Red))
            } else {
                Span::styled("[OK]", Style::default().fg(Color::Green))
            };

            // Name style
            let name_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else if plugin.session_disabled {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(state.theme.foreground)
            };

            // Version
            let version = Span::styled(
                format!("v{}", plugin.version),
                Style::default().fg(Color::Cyan),
            );

            // Description
            let desc = Span::styled(
                &plugin.description,
                Style::default().fg(Color::DarkGray),
            );

            let line = Line::from(vec![
                Span::raw(" "),
                status,
                Span::raw(" "),
                Span::styled(&plugin.name, name_style),
                Span::raw(" "),
                version,
                Span::raw(" - "),
                desc,
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).style(Style::default().fg(state.theme.foreground));
    f.render_widget(list, area);
}

/// Render the marketplace plugins list
#[allow(clippy::too_many_arguments)]
fn render_marketplace_list(
    f: &mut Frame,
    state: &AppState,
    area: Rect,
    selected_index: usize,
    plugins: Option<&[PluginEntry]>,
    loading: bool,
    error: Option<&str>,
    marketplace_name: &str,
) {
    // Loading state
    if loading {
        let spinner_chars = ['/', '-', '\\', '|'];
        let spinner = spinner_chars[state.spinner_frame % spinner_chars.len()];
        let loading_msg = Paragraph::new(Line::from(vec![
            Span::styled(format!("{} ", spinner), Style::default().fg(Color::Yellow)),
            Span::styled(
                "Loading marketplace...",
                Style::default().fg(state.theme.foreground),
            ),
        ]));
        f.render_widget(loading_msg, area);
        return;
    }

    // Error state
    if let Some(err) = error {
        let error_msg = Paragraph::new(vec![
            Line::from(Span::styled(
                "Failed to load marketplace:",
                Style::default().fg(Color::Red),
            )),
            Line::from(Span::styled(err, Style::default().fg(Color::DarkGray))),
            Line::from(""),
            Line::from(Span::styled(
                "Press Tab to switch to Installed tab.",
                Style::default().fg(Color::Yellow),
            )),
        ])
        .wrap(Wrap { trim: true });
        f.render_widget(error_msg, area);
        return;
    }

    // Not yet loaded (switch to Marketplace tab to trigger load)
    let plugins = match plugins {
        Some(p) => p,
        None => {
            let not_loaded = Paragraph::new(Line::from(Span::styled(
                "Press Tab to load marketplace plugins...",
                Style::default().fg(Color::DarkGray),
            )));
            f.render_widget(not_loaded, area);
            return;
        }
    };

    // Empty marketplace
    if plugins.is_empty() {
        let empty_msg = Paragraph::new(Line::from(Span::styled(
            "No plugins available in the marketplace.",
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(empty_msg, area);
        return;
    }

    // Split area into header + list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    // Render marketplace header
    let header = Line::from(vec![
        Span::styled("── ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            marketplace_name,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ──", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    let items: Vec<ListItem> = plugins
        .iter()
        .enumerate()
        .map(|(i, plugin)| {
            let is_selected = i == selected_index;
            // Check if plugin is installed on disk (not just loaded)
            let is_installed = crate::plugin::PluginManager::is_plugin_installed(&plugin.name);

            // Status indicator
            let status = if is_installed {
                Span::styled("[installed]", Style::default().fg(Color::Green))
            } else {
                Span::styled("[available]", Style::default().fg(Color::Blue))
            };

            // Name style
            let name_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(state.theme.foreground)
            };

            // Version
            let version = Span::styled(
                format!("v{}", plugin.version),
                Style::default().fg(Color::Cyan),
            );

            // Description
            let desc = Span::styled(
                &plugin.description,
                Style::default().fg(Color::DarkGray),
            );

            let line = Line::from(vec![
                Span::raw(" "),
                status,
                Span::raw(" "),
                Span::styled(&plugin.name, name_style),
                Span::raw(" "),
                version,
                Span::raw(" - "),
                desc,
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).style(Style::default().fg(state.theme.foreground));
    f.render_widget(list, chunks[1]);
}

/// Render the plugin details view
fn render_details_view(f: &mut Frame, state: &AppState, plugin: &PluginEntry) {
    let area = centered_rect(60, 50, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", plugin.name))
        .style(Style::default().bg(state.theme.background));

    f.render_widget(block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 2,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(4),
    };

    // Check if already installed (on disk, not just loaded)
    let is_installed = crate::plugin::PluginManager::is_plugin_installed(&plugin.name);

    let lines = vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&plugin.name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Version: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("v{}", plugin.version), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Description: ", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(Span::styled(&plugin.description, Style::default().fg(state.theme.foreground))),
        Line::from(""),
        Line::from(""),
        if is_installed {
            Line::from(Span::styled(
                "✓ Already installed",
                Style::default().fg(Color::Green),
            ))
        } else {
            Line::from(vec![
                Span::styled("Press ", Style::default().fg(Color::DarkGray)),
                Span::styled("[i]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(" or ", Style::default().fg(Color::DarkGray)),
                Span::styled("[Enter]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(" to install", Style::default().fg(Color::DarkGray)),
            ])
        },
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
            Span::styled(" to go back", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(paragraph, inner);
}

/// Render the input view for plugin invocation
fn render_input_view(
    f: &mut Frame,
    state: &AppState,
    plugin_name: &str,
    input_buffer: &str,
    cursor_pos: usize,
) {
    let area = centered_rect(60, 20, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} - Enter input (Esc to go back) ", plugin_name))
        .style(Style::default().bg(state.theme.background));

    f.render_widget(block, area);

    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Render cursor within input
    let before_cursor = &input_buffer[..cursor_pos];
    let after_cursor = &input_buffer[cursor_pos..];

    let cursor_char = if after_cursor.is_empty() {
        "█"
    } else {
        &after_cursor[..after_cursor
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0)]
    };

    let after_cursor_rest = if after_cursor.is_empty() {
        ""
    } else {
        &after_cursor[after_cursor
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0)..]
    };

    let input_line = Line::from(vec![
        Span::raw(before_cursor),
        Span::styled(
            cursor_char,
            Style::default().bg(Color::Yellow).fg(Color::Black),
        ),
        Span::raw(after_cursor_rest),
    ]);

    let input_paragraph = Paragraph::new(input_line);
    f.render_widget(input_paragraph, inner_area);
}

/// Render the select input view for Select type config fields
fn render_select_input_view(
    f: &mut Frame,
    state: &AppState,
    plugin_name: &str,
    field_name: &str,
    options: &[(String, String)],
    selected_index: usize,
) {
    let area = centered_rect(60, 50, f.area());

    // Clear background
    f.render_widget(Clear, area);

    // Build title
    let title = format!(" {} - Select {} ", plugin_name, field_name);

    // Create main block
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().bg(state.theme.background));

    f.render_widget(block, area);

    // Inner area for content
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Layout: options list + footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Options list
            Constraint::Length(1), // Footer
        ])
        .split(inner);

    // Render options list
    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, (display, _value))| {
            let is_selected = i == selected_index;

            let name_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(state.theme.foreground)
            };

            ListItem::new(Line::from(Span::styled(format!(" {} ", display), name_style)))
        })
        .collect();

    let list = List::new(items).style(Style::default().fg(state.theme.foreground));
    f.render_widget(list, chunks[0]);

    // Render footer
    let footer = Paragraph::new(Line::from(Span::styled(
        "[j/k] navigate | [Enter] select | [Esc] cancel",
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(footer, chunks[1]);
}

/// Render the executing view with spinner
fn render_executing_view(f: &mut Frame, state: &AppState, plugin_name: &str) {
    let area = centered_rect(40, 15, f.area());

    f.render_widget(Clear, area);

    let spinner = state.get_spinner_char();
    let text = format!("{} Running {}...\n\nPlease wait.", spinner, plugin_name);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Executing Plugin ")
        .style(Style::default().bg(state.theme.background));

    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(state.theme.foreground))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render the preview view for generated items
fn render_preview_view(f: &mut Frame, state: &AppState, items: &[crate::todo::TodoItem]) {
    let area = centered_rect(70, 60, f.area());

    f.render_widget(Clear, area);

    let list_items: Vec<ListItem> = items
        .iter()
        .map(|item| {
            let indent = "  ".repeat(item.indent_level);
            let line = format!("{}[ ] {}", indent, item.content);
            ListItem::new(Line::from(Span::styled(
                line,
                Style::default().fg(state.theme.foreground),
            )))
        })
        .collect();

    let title = format!(" Generated {} item(s) - Add to list? (Y/n) ", items.len());

    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(list, area);
}

/// Render the error view
fn render_error_view(f: &mut Frame, state: &AppState, message: &str) {
    let area = centered_rect(60, 30, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Error (Press Esc to dismiss) ")
        .style(
            Style::default()
                .bg(state.theme.background)
                .fg(Color::Red),
        );

    let paragraph = Paragraph::new(message)
        .block(block)
        .style(Style::default().fg(Color::Red))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Create a centered rectangle with given percentage of width and height
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
