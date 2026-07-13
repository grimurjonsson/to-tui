pub mod description_modal;
pub mod plugin_modal;
pub mod status_bar;
pub mod todo_list;

use crate::app::AppState;
use crate::app::mode::Mode;
use crate::app::state::{MoveToProjectSubState, PluginSubState, ProjectSubState};
use crate::keybindings::Action;
use crate::project::DEFAULT_PROJECT_NAME;
use crate::utils::upgrade::{PluginUpgradeSubState, UpgradeSubState, format_bytes};
use chrono::{Local, NaiveDate};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn render(f: &mut Frame, state: &mut AppState) {
    // Update terminal dimensions for click and scroll calculations
    state.terminal_width = f.area().width;
    state.terminal_height = f.area().height;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Todo list
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    // Render todo list
    todo_list::render(f, state, chunks[0]);

    // Render status bar
    status_bar::render(f, state, chunks[1]);

    if state.show_help {
        render_help_overlay(f, state);
    }

    // Render new plugins modal if active, otherwise fall back to old plugin overlay
    if state.plugins_modal_state.is_some() {
        plugin_modal::render_plugins_modal(f, state);
    } else if let Some(ref plugin_state) = state.plugin_state {
        render_plugin_overlay(f, state, plugin_state);
    }

    if state.mode == Mode::Rollover {
        render_rollover_overlay(f, state);
    }

    // Render plugin error popup overlay
    if state.show_plugin_error_popup {
        render_plugin_error_popup(f, state);
    }

    if state.mode == Mode::UpgradePrompt {
        render_upgrade_overlay(f, state);
    }

    if state.mode == Mode::ProjectSelect
        && let Some(ref project_state) = state.project_state
    {
        render_project_overlay(f, state, project_state);
    }

    if state.mode == Mode::MoveToProject {
        render_move_to_project_modal(f, state);
    }

    if state.mode == Mode::EditDescription {
        description_modal::render_description_modal(f, state);
    }
}

#[derive(Clone, Copy)]
enum HelpBindingScope {
    Navigate,
    Edit,
    Visual,
}

fn help_binding(
    state: &AppState,
    scope: HelpBindingScope,
    action: Action,
    style: Style,
) -> Span<'static> {
    let label = help_binding_text(state, scope, action);
    let padding = " ".repeat(16usize.saturating_sub(UnicodeWidthStr::width(label.as_str())));
    Span::styled(format!("    {label}{padding}  "), style)
}

fn help_binding_text(state: &AppState, scope: HelpBindingScope, action: Action) -> String {
    let bindings = match scope {
        HelpBindingScope::Navigate => state.keybindings.navigate_bindings_for(action),
        HelpBindingScope::Edit => state.keybindings.edit_bindings_for(action),
        HelpBindingScope::Visual => state.keybindings.visual_bindings_for(action),
    };

    bindings
        .filter(|bindings| !bindings.is_empty())
        .map(|bindings| bindings.join(" / "))
        .unwrap_or_else(|| "(unbound)".to_string())
}

#[allow(clippy::vec_init_then_push)]
fn render_help_overlay(f: &mut Frame, state: &mut AppState) {
    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(state.theme.foreground);
    let section_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(Color::DarkGray);

    let mut lines: Vec<Line> = vec![];

    // Title
    lines.push(Line::from(vec![Span::styled(
        "  TO-TUI Help",
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "  ── Essentials ──",
        section_style,
    )));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::NewItem,
            key_style,
        ),
        Span::styled("Add a todo", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::ToggleState,
            key_style,
        ),
        Span::styled("Mark done / undone", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Navigate, Action::Delete, key_style),
        Span::styled("Delete a todo", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::ToggleHelp,
            key_style,
        ),
        Span::styled("Open or close this help", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Navigate, Action::Quit, key_style),
        Span::styled("Quit", desc_style),
    ]));
    lines.push(Line::from(""));

    // Navigation section
    lines.push(Line::from(Span::styled(
        "  ── Navigation ──",
        section_style,
    )));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::MoveDown,
            key_style,
        ),
        Span::styled("Move cursor down", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Navigate, Action::MoveUp, key_style),
        Span::styled("Move cursor up", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::CollapseOrParent,
            key_style,
        ),
        Span::styled("Collapse item or go to parent", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Navigate, Action::Expand, key_style),
        Span::styled("Expand collapsed item", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::ToggleCollapse,
            key_style,
        ),
        Span::styled("Toggle collapse/expand", desc_style),
    ]));
    lines.push(Line::from(""));

    // Item State section
    lines.push(Line::from(Span::styled(
        "  ── Item State ──",
        section_style,
    )));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::ToggleState,
            key_style,
        ),
        Span::styled("Toggle done/undone", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::CycleState,
            key_style,
        ),
        Span::styled("Cycle: [ ]→[x]→[*]→[?]→[!]→[-]", desc_style),
    ]));
    lines.push(Line::from(""));

    // Item Management section
    lines.push(Line::from(Span::styled(
        "  ── Item Management ──",
        section_style,
    )));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::NewItem,
            key_style,
        ),
        Span::styled("New item below", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::InsertItemAbove,
            key_style,
        ),
        Span::styled("New item above", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::NewItemSameLevel,
            key_style,
        ),
        Span::styled("New item at same indent level", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::EnterEditMode,
            key_style,
        ),
        Span::styled("Edit current item", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::EditDescription,
            key_style,
        ),
        Span::styled("Edit description", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Navigate, Action::Delete, key_style),
        Span::styled("Delete item (with children)", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Navigate, Action::Yank, key_style),
        Span::styled("Yank (copy) item to clipboard", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Navigate, Action::Undo, key_style),
        Span::styled("Undo last action", desc_style),
    ]));
    lines.push(Line::from(""));

    // Indentation section
    lines.push(Line::from(Span::styled(
        "  ── Indentation ──",
        section_style,
    )));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Navigate, Action::Indent, key_style),
        Span::styled("Indent item", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::Outdent,
            key_style,
        ),
        Span::styled("Outdent item", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::IndentWithChildren,
            key_style,
        ),
        Span::styled("Indent with children", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::OutdentWithChildren,
            key_style,
        ),
        Span::styled("Outdent with children", desc_style),
    ]));
    lines.push(Line::from(""));

    // Move Items section
    lines.push(Line::from(Span::styled(
        "  ── Move Items ──",
        section_style,
    )));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::MoveItemUp,
            key_style,
        ),
        Span::styled("Move item up (with children)", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::MoveItemDown,
            key_style,
        ),
        Span::styled("Move item down (with children)", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::MoveToProject,
            key_style,
        ),
        Span::styled("Move to destination's today (+ children)", desc_style),
    ]));
    lines.push(Line::from(""));

    // Priority section
    lines.push(Line::from(Span::styled("  ── Priority ──", section_style)));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::CyclePriority,
            key_style,
        ),
        Span::styled("Cycle priority: none→low→medium→high", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::SortByPriority,
            key_style,
        ),
        Span::styled("Sort items by priority", desc_style),
    ]));
    lines.push(Line::from(""));

    // Visual Mode section
    lines.push(Line::from(Span::styled(
        "  ── Visual Mode ──",
        section_style,
    )));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::ToggleVisual,
            key_style,
        ),
        Span::styled("Enter visual mode (select multiple)", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Visual,
            Action::ExitVisual,
            key_style,
        ),
        Span::styled("Exit visual mode", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(
            format!(
                "In visual: {} / {} extend selection; {} / {} indent/outdent",
                help_binding_text(state, HelpBindingScope::Visual, Action::MoveUp),
                help_binding_text(state, HelpBindingScope::Visual, Action::MoveDown),
                help_binding_text(state, HelpBindingScope::Visual, Action::Indent),
                help_binding_text(state, HelpBindingScope::Visual, Action::Outdent),
            ),
            dim_style,
        ),
    ]));
    lines.push(Line::from(""));

    // Day Navigation section
    lines.push(Line::from(Span::styled(
        "  ── Day Navigation ──",
        section_style,
    )));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::PrevDay,
            key_style,
        ),
        Span::styled("Previous day (archived, readonly)", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::NextDay,
            key_style,
        ),
        Span::styled("Next day", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::GoToToday,
            key_style,
        ),
        Span::styled("Go to today", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::OpenRolloverModal,
            key_style,
        ),
        Span::styled("Open rollover modal", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled("Past dates are read-only; use ", dim_style),
        Span::styled(
            help_binding_text(state, HelpBindingScope::Navigate, Action::GoToToday),
            key_style,
        ),
        Span::styled(" to edit.", dim_style),
    ]));
    lines.push(Line::from(""));

    // Other section
    lines.push(Line::from(Span::styled("  ── Other ──", section_style)));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::OpenProjectModal,
            key_style,
        ),
        Span::styled("Open project switcher", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::OpenPluginMenu,
            key_style,
        ),
        Span::styled("Open plugins menu", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::CopyLogPath,
            key_style,
        ),
        Span::styled("Copy log file path to clipboard", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Navigate,
            Action::ToggleHelp,
            key_style,
        ),
        Span::styled("Toggle this help", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Navigate, Action::Quit, key_style),
        Span::styled("Quit", desc_style),
    ]));
    lines.push(Line::from(""));

    // Edit Mode section
    lines.push(Line::from(Span::styled("  ── Edit Mode ──", section_style)));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Edit, Action::EditCancel, key_style),
        Span::styled("Save and exit edit mode", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Edit,
            Action::EditConfirm,
            key_style,
        ),
        Span::styled("Save and create new item below", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Edit, Action::EditLeft, key_style),
        Span::styled("Move cursor", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Edit, Action::EditRight, key_style),
        Span::styled("Move cursor right", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Edit,
            Action::EditWordLeft,
            key_style,
        ),
        Span::styled("Move word left", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Edit,
            Action::EditWordRight,
            key_style,
        ),
        Span::styled("Move word right", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Edit, Action::EditHome, key_style),
        Span::styled("Go to start of line", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Edit, Action::EditEnd, key_style),
        Span::styled("Go to end of line", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(state, HelpBindingScope::Edit, Action::EditIndent, key_style),
        Span::styled("Indent/outdent while editing", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Edit,
            Action::EditOutdent,
            key_style,
        ),
        Span::styled("Outdent while editing", desc_style),
    ]));
    lines.push(Line::from(vec![
        help_binding(
            state,
            HelpBindingScope::Edit,
            Action::EditBackspace,
            key_style,
        ),
        Span::styled("Delete character", desc_style),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  ── Modal Controls ──",
        section_style,
    )));
    lines.push(Line::from(Span::styled(
        "    Project picker",
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("    ↑/↓ / j/k       ", key_style),
        Span::styled("Select project", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Enter           ", key_style),
        Span::styled("Switch project", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    n / r / d       ", key_style),
        Span::styled("Create / rename / delete project", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Esc / q         ", key_style),
        Span::styled("Close picker", desc_style),
    ]));
    lines.push(Line::from(Span::styled(
        "    Move to project",
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("    ↑/↓ / j/k       ", key_style),
        Span::styled("Select destination", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Enter           ", key_style),
        Span::styled("Move item and children", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Esc / q         ", key_style),
        Span::styled("Cancel", desc_style),
    ]));
    lines.push(Line::from(Span::styled(
        "    Rollover confirmation",
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("    y / Enter       ", key_style),
        Span::styled("Roll over incomplete items", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    n / Esc         ", key_style),
        Span::styled("Keep the current list", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Tab / Space     ", key_style),
        Span::styled("Toggle “remember choice”", desc_style),
    ]));
    lines.push(Line::from(Span::styled(
        "    Plugins",
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("    Tab / Shift+Tab ", key_style),
        Span::styled("Switch Installed and Marketplace tabs", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    ↑/↓ / j/k       ", key_style),
        Span::styled("Select plugin", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Enter           ", key_style),
        Span::styled(
            "Run installed plugin or open marketplace details",
            desc_style,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Esc / q         ", key_style),
        Span::styled("Close plugins", desc_style),
    ]));

    // Plugin Actions section (only if any enabled plugins have actions)
    let actions_by_plugin = state.plugin_action_registry.actions_by_plugin();
    if !actions_by_plugin.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ── Plugin Actions ──",
            section_style,
        )));

        // Sort plugin names for consistent display
        let mut plugin_names: Vec<_> = actions_by_plugin.keys().collect();
        plugin_names.sort();

        for plugin_name in plugin_names {
            let actions = &actions_by_plugin[plugin_name];

            lines.push(Line::from(vec![Span::styled(
                format!("  [{}]", plugin_name),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )]));

            for action in actions {
                // Format the KeyBinding directly using its Display impl
                let key_text = action
                    .keybinding
                    .as_ref()
                    .map(|kb| format!("{:<16}", kb))
                    .unwrap_or_else(|| "(no binding)    ".to_string());

                lines.push(Line::from(vec![
                    Span::styled(format!("    {}  ", key_text), key_style),
                    Span::styled(&action.description, desc_style),
                ]));
            }
        }
    }

    lines.push(Line::from(""));

    // Footer hint
    lines.push(Line::from(vec![Span::styled(
        "  ↑/↓ or j/k line • PgUp/PgDn page • Home/End jump • Esc or ? close",
        dim_style,
    )]));

    let area = centered_rect(65, 80, f.area());
    let metrics_block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .title_bottom(Line::from(" Help navigation ").centered())
        .style(Style::default().bg(state.theme.background));
    let inner = metrics_block.inner(area);
    let lines = wrap_help_lines(lines, inner.width);
    let (total_lines, max_scroll) = help_scroll_metrics(&lines, inner);
    state.help_max_scroll = max_scroll;
    state.help_page_size = inner.height.max(1);
    state.help_scroll = state.help_scroll.min(max_scroll);
    let current_line = state.help_scroll.saturating_add(1).min(total_lines);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .title_bottom(
            Line::from(format!(
                " {current_line}/{total_lines} · ↑↓ PgUp/Dn Home/End "
            ))
            .centered(),
        )
        .style(Style::default().bg(state.theme.background));

    let help_widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((state.help_scroll, 0))
        .block(block);

    f.render_widget(Clear, area);
    f.render_widget(help_widget, area);

    // Render scrollbar if content exceeds viewport
    if max_scroll > 0 {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state =
            ScrollbarState::new(max_scroll as usize + 1).position(state.help_scroll as usize);

        f.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

fn help_max_scroll(lines: &[Line], inner: Rect) -> u16 {
    help_scroll_metrics(lines, inner).1
}

fn help_scroll_metrics(lines: &[Line], inner: Rect) -> (u16, u16) {
    let paragraph = Paragraph::new(lines.to_vec()).wrap(Wrap { trim: false });
    let total_lines = paragraph.line_count(inner.width).min(u16::MAX as usize) as u16;
    let max_scroll = total_lines.saturating_sub(inner.height);
    (total_lines, max_scroll)
}

fn wrap_help_lines<'a>(lines: Vec<Line<'a>>, width: u16) -> Vec<Line<'a>> {
    lines
        .into_iter()
        .flat_map(|line| wrap_help_line(line, width))
        .collect()
}

fn wrap_help_line<'a>(line: Line<'a>, width: u16) -> Vec<Line<'a>> {
    if line.spans.len() == 2 && line.spans[0].content.starts_with("    ") {
        return wrap_keyed_help_line(line, width);
    }

    let Some(indent_width) = line
        .spans
        .first()
        .filter(|span| !span.content.is_empty() && span.content.chars().all(char::is_whitespace))
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
    else {
        return vec![line];
    };

    wrap_hanging_help_line(line, width, indent_width)
}

fn wrap_keyed_help_line<'a>(line: Line<'a>, width: u16) -> Vec<Line<'a>> {
    let key = line.spans[0].clone();
    let description = line.spans[1].clone();
    let key_width = UnicodeWidthStr::width(key.content.as_ref());
    let available_width = width as usize;
    if available_width <= key_width {
        return vec![line];
    }

    let description_lines =
        wrap_description(description.content.as_ref(), available_width - key_width);
    let Some((first, rest)) = description_lines.split_first() else {
        return vec![line];
    };

    let mut wrapped = vec![Line::from(vec![
        key,
        Span::styled(first.clone(), description.style),
    ])];
    let indent = " ".repeat(key_width);

    wrapped.extend(
        rest.iter()
            .map(|text| Line::from(Span::styled(format!("{indent}{text}"), description.style))),
    );

    wrapped
}

fn wrap_hanging_help_line<'a>(line: Line<'a>, width: u16, indent_width: usize) -> Vec<Line<'a>> {
    if width as usize <= indent_width {
        return vec![line];
    }

    let words: Vec<_> = line.spans[1..]
        .iter()
        .flat_map(|span| {
            span.content
                .split_whitespace()
                .map(|word| (word.to_string(), span.style))
        })
        .collect();
    if words.is_empty() {
        return vec![line];
    }

    let mut wrapped = Vec::new();
    let indent = " ".repeat(indent_width);
    let mut line = vec![Span::raw(indent.clone())];
    let mut line_width = indent_width;

    for (word, style) in words {
        for (index, chunk) in split_word(&word, width as usize - indent_width)
            .into_iter()
            .enumerate()
        {
            let separator = if index == 0 && line_width > indent_width {
                " "
            } else {
                ""
            };
            let candidate_width = line_width
                + UnicodeWidthStr::width(separator)
                + UnicodeWidthStr::width(chunk.as_str());

            if line_width > indent_width && candidate_width > width as usize {
                wrapped.push(Line::from(line));
                line = vec![Span::raw(indent.clone())];
                line_width = indent_width;
            } else if !separator.is_empty() {
                line.push(Span::raw(" "));
                line_width += 1;
            }

            line_width += UnicodeWidthStr::width(chunk.as_str());
            line.push(Span::styled(chunk, style));
        }
    }

    wrapped.push(Line::from(line));
    wrapped
}

fn wrap_description(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let chunks = split_word(word, max_width);
        for (index, chunk) in chunks.iter().enumerate() {
            let separator = if index == 0 && !current.is_empty() {
                " "
            } else {
                ""
            };
            let candidate_width = UnicodeWidthStr::width(current.as_str())
                + UnicodeWidthStr::width(separator)
                + UnicodeWidthStr::width(chunk.as_str());

            if !current.is_empty() && candidate_width > max_width {
                lines.push(std::mem::take(&mut current));
            } else if !separator.is_empty() {
                current.push(' ');
            }

            current.push_str(chunk);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn split_word(word: &str, max_width: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut chunk = String::new();
    let mut chunk_width = 0;

    for character in word.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if !chunk.is_empty() && chunk_width + character_width > max_width {
            chunks.push(std::mem::take(&mut chunk));
            chunk_width = 0;
        }
        chunk.push(character);
        chunk_width += character_width;
    }

    if !chunk.is_empty() {
        chunks.push(chunk);
    }

    chunks
}

#[cfg(test)]
mod help_tests {
    use super::*;

    #[test]
    fn help_max_scroll_counts_wrapped_lines() {
        let lines = vec![Line::from(
            "A long help description that wraps across several narrow terminal rows",
        )];

        assert_eq!(help_max_scroll(&lines, Rect::new(0, 0, 80, 3)), 0);
        assert!(help_max_scroll(&lines, Rect::new(0, 0, 20, 3)) > 0);
    }

    #[test]
    fn help_scroll_metrics_report_total_and_maximum() {
        let lines = vec![
            Line::from("first"),
            Line::from("second"),
            Line::from("third"),
        ];

        assert_eq!(help_scroll_metrics(&lines, Rect::new(0, 0, 80, 2)), (3, 1));
    }

    #[test]
    fn help_entries_wrap_with_a_hanging_indent() {
        let key = "    Alt+Shift+↑     ";
        let lines = wrap_help_lines(
            vec![Line::from(vec![
                Span::styled(key, Style::default().fg(Color::Yellow)),
                Span::raw("Move item up with all of its children"),
            ])],
            30,
        );

        assert!(lines.len() > 1);
        assert_eq!(lines[0].spans[0].content, key);
        assert!(
            lines[1].spans[0]
                .content
                .starts_with(&" ".repeat(UnicodeWidthStr::width(key)))
        );
    }

    #[test]
    fn styled_help_entries_preserve_their_hanging_indent() {
        let lines = wrap_help_lines(
            vec![Line::from(vec![
                Span::raw("    "),
                Span::raw("In visual: "),
                Span::styled("j/k", Style::default().fg(Color::Yellow)),
                Span::raw(" extend selection, "),
                Span::styled("Tab/S-Tab", Style::default().fg(Color::Yellow)),
                Span::raw(" indent/outdent"),
            ])],
            20,
        );

        assert!(lines.len() > 1);
        assert!(lines.iter().all(|line| line.spans[0].content == "    "));
    }
}

pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

/// Create a centered rect with percentage width and absolute height in lines
fn centered_rect_absolute_height(percent_x: u16, height: u16, r: Rect) -> Rect {
    let height = height.min(r.height); // Don't exceed available height
    let vertical_margin = (r.height.saturating_sub(height)) / 2;

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(vertical_margin),
            Constraint::Length(height),
            Constraint::Length(r.height.saturating_sub(vertical_margin + height)),
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

fn render_plugin_overlay(f: &mut Frame, state: &AppState, plugin_state: &PluginSubState) {
    match plugin_state {
        PluginSubState::Selecting {
            plugins,
            selected_index,
        } => render_plugin_selecting(f, state, plugins, *selected_index),
        PluginSubState::InputPrompt {
            plugin_name,
            input_buffer,
            cursor_pos,
        } => render_plugin_input(f, state, plugin_name, input_buffer, *cursor_pos),
        PluginSubState::Executing { plugin_name } => render_plugin_executing(f, state, plugin_name),
        PluginSubState::Error { message } => render_plugin_error(f, state, message),
        PluginSubState::Preview { items } => render_plugin_preview(f, state, items),
    }
}

fn render_plugin_selecting(
    f: &mut Frame,
    state: &AppState,
    plugins: &[crate::plugin::GeneratorInfo],
    selected_index: usize,
) {
    let area = centered_rect(50, 40, f.area());

    let items: Vec<ListItem> = plugins
        .iter()
        .enumerate()
        .map(|(i, plugin)| {
            let status = if plugin.available {
                Span::styled("[OK]", Style::default().fg(ratatui::style::Color::Green))
            } else {
                Span::styled("[N/A]", Style::default().fg(ratatui::style::Color::Red))
            };

            let name_style = if i == selected_index {
                Style::default()
                    .fg(ratatui::style::Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else if plugin.available {
                Style::default().fg(state.theme.foreground)
            } else {
                Style::default().fg(ratatui::style::Color::DarkGray)
            };

            let line = Line::from(vec![
                Span::styled(format!(" {} ", plugin.name), name_style),
                status,
                Span::raw(" "),
                Span::styled(
                    &plugin.description,
                    Style::default().fg(ratatui::style::Color::Gray),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Plugins (Enter to select, Esc to cancel) ")
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(list, area);
}

fn render_plugin_input(
    f: &mut Frame,
    state: &AppState,
    plugin_name: &str,
    input_buffer: &str,
    cursor_pos: usize,
) {
    let area = centered_rect(60, 20, f.area());

    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {plugin_name} - Enter input (Esc to go back) "))
        .style(Style::default().bg(state.theme.background));

    f.render_widget(Clear, area);
    f.render_widget(block, area);

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
            Style::default()
                .bg(ratatui::style::Color::Yellow)
                .fg(ratatui::style::Color::Black),
        ),
        Span::raw(after_cursor_rest),
    ]);

    let input_paragraph = Paragraph::new(input_line);
    f.render_widget(input_paragraph, inner_area);
}

fn render_plugin_executing(f: &mut Frame, state: &AppState, plugin_name: &str) {
    let area = centered_rect(40, 15, f.area());

    let spinner = state.get_spinner_char();
    let text = format!("{spinner} Running {plugin_name}...\n\nPlease wait. (Esc to cancel)");

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Executing Plugin ")
        .style(Style::default().bg(state.theme.background));

    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(state.theme.foreground))
        .wrap(Wrap { trim: true });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn render_plugin_error(f: &mut Frame, state: &AppState, message: &str) {
    let area = centered_rect(60, 30, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Error (Press Esc to dismiss) ")
        .style(
            Style::default()
                .bg(state.theme.background)
                .fg(ratatui::style::Color::Red),
        );

    let paragraph = Paragraph::new(message)
        .block(block)
        .style(Style::default().fg(ratatui::style::Color::Red))
        .wrap(Wrap { trim: true });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn render_plugin_preview(f: &mut Frame, state: &AppState, items: &[crate::todo::TodoItem]) {
    let area = centered_rect(70, 60, f.area());

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

    f.render_widget(Clear, area);
    f.render_widget(list, area);
}

fn format_date_description(source_date: NaiveDate) -> String {
    let today = Local::now().date_naive();
    let days_ago = (today - source_date).num_days();

    if days_ago == 1 {
        "yesterday".to_string()
    } else {
        format!(
            "{} ({} days ago)",
            source_date.format("%B %d, %Y"),
            days_ago
        )
    }
}

fn render_rollover_overlay(f: &mut Frame, state: &AppState) {
    let Some(ref pending) = state.pending_rollover else {
        return;
    };

    let area = centered_rect(60, 50, f.area());

    let date_desc = format_date_description(pending.source_date);
    let today_desc = Local::now().date_naive().format("%B %d, %Y").to_string();
    let item_count = pending.items.len();
    let title = format!(" Rollover ({} items) ", item_count);

    // Build content: description header + item list
    let mut lines: Vec<ListItem> = Vec::new();

    // Description of what rollover does
    let desc_style = Style::default().fg(state.theme.foreground);
    lines.push(ListItem::new(Line::from(Span::styled(
        format!(
            "Move {} incomplete item{} from {} to today ({}).",
            item_count,
            if item_count == 1 { "" } else { "s" },
            date_desc,
            today_desc,
        ),
        desc_style,
    ))));
    lines.push(ListItem::new(Line::from(Span::styled(
        format!("Old items from {} will be archived.", date_desc),
        Style::default().fg(Color::DarkGray),
    ))));
    lines.push(ListItem::new(Line::from("")));

    // Section header
    lines.push(ListItem::new(Line::from(Span::styled(
        "Items to rollover:",
        Style::default()
            .fg(state.theme.foreground)
            .add_modifier(Modifier::BOLD),
    ))));

    // Item list
    for item in &pending.items {
        let indent = "  ".repeat(item.indent_level);
        let state_char = item.state.to_char();
        let line = format!("  {}[{}] {}", indent, state_char, item.content);
        lines.push(ListItem::new(Line::from(Span::styled(
            line,
            Style::default().fg(state.theme.foreground),
        ))));
    }

    // Spacer + "Don't ask again" checkbox row
    lines.push(ListItem::new(Line::from("")));
    let checkbox_glyph = if pending.remember_choice {
        "[x]"
    } else {
        "[ ]"
    };
    let checkbox_style = if pending.remember_choice {
        Style::default()
            .fg(state.theme.foreground)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(state.theme.foreground)
    };
    lines.push(ListItem::new(Line::from(vec![
        Span::raw("  "),
        Span::styled(checkbox_glyph, checkbox_style),
        Span::raw(" Don't ask me again — remember this choice (Tab to toggle)"),
    ])));

    let list = List::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(list, area);

    // Render footer with options
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Y]",
            Style::default()
                .fg(ratatui::style::Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("es    "),
        Span::styled(
            "[N]",
            Style::default()
                .fg(ratatui::style::Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("o    "),
        Span::styled(
            "[Tab]",
            Style::default()
                .fg(ratatui::style::Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" remember    "),
        Span::styled(
            "[Esc]",
            Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" cancel"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_upgrade_overlay(f: &mut Frame, state: &AppState) {
    // Check if there are any updates available (app or plugins)
    if state.new_version_available.is_none() && state.plugin_updates_available.is_empty() {
        return;
    }

    let sub_state = state.upgrade_sub_state.as_ref();

    match sub_state {
        Some(UpgradeSubState::Downloading {
            progress,
            bytes_downloaded,
            total_bytes,
        }) => {
            render_upgrade_downloading(f, state, *progress, *bytes_downloaded, *total_bytes);
        }
        Some(UpgradeSubState::Error { message }) => {
            render_upgrade_error(f, state, message);
        }
        Some(UpgradeSubState::RestartPrompt { downloaded_path: _ }) => {
            render_upgrade_restart_prompt(f, state);
        }
        Some(UpgradeSubState::PluginUpgrades(plugin_sub_state)) => {
            render_plugin_upgrade_overlay(f, state, plugin_sub_state);
        }
        Some(UpgradeSubState::Prompt) | None => {
            render_upgrade_prompt(f, state);
        }
    }
}

fn render_upgrade_prompt(f: &mut Frame, state: &AppState) {
    let has_app_update = state.new_version_available.is_some();
    let has_plugin_updates = !state.plugin_updates_available.is_empty();

    // Calculate height based on content lines:
    //   1 empty line at start
    //   App section (if present): 4 lines (header + current + new + empty)
    //   Plugin section (if present): 1 header + N items (max 5) + maybe 1 "...and N more" + 1 empty
    //   1 footer line
    // Plus: 2 for borders (top/bottom)
    let content_lines = 1; // initial empty line
    let app_section_lines = if has_app_update { 4 } else { 0 };
    let plugin_items = state.plugin_updates_available.len().min(5);
    let plugin_overflow = if state.plugin_updates_available.len() > 5 {
        1
    } else {
        0
    };
    let plugin_section_lines = if has_plugin_updates {
        1 + plugin_items + plugin_overflow + 1 // header + items + overflow + trailing empty
    } else {
        0
    };
    let footer_line = 1;
    // Total: content + borders
    let total_height = content_lines + app_section_lines + plugin_section_lines + footer_line + 2;

    let area = centered_rect_absolute_height(60, total_height as u16, f.area());

    let current_version = env!("CARGO_PKG_VERSION");

    let title = if has_app_update && has_plugin_updates {
        " Updates Available "
    } else if has_app_update {
        " New Version Available "
    } else {
        " Plugin Updates Available "
    };

    // Build content lines
    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));

    // App update section
    if has_app_update {
        let new_version = state.new_version_available.as_ref().unwrap();
        lines.push(Line::from(vec![Span::styled(
            "  App Update:",
            Style::default().add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![
            Span::raw("    Current: "),
            Span::styled(
                format!("v{}", current_version),
                Style::default().fg(Color::Yellow),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("    New:     "),
            Span::styled(
                format!("v{}", new_version),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
    }

    // Plugin updates section
    if has_plugin_updates {
        lines.push(Line::from(vec![Span::styled(
            format!(
                "  Plugin Updates ({}):",
                state.plugin_updates_available.len()
            ),
            Style::default().add_modifier(Modifier::BOLD),
        )]));
        for plugin in state.plugin_updates_available.iter().take(5) {
            lines.push(Line::from(vec![
                Span::raw("    • "),
                Span::styled(plugin.plugin_name.clone(), Style::default().fg(Color::Cyan)),
                Span::raw(": "),
                Span::styled(
                    plugin.current_version.clone(),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(" → "),
                Span::styled(
                    plugin.latest_version.clone(),
                    Style::default().fg(Color::Green),
                ),
            ]));
        }
        if state.plugin_updates_available.len() > 5 {
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "    ...and {} more",
                    state.plugin_updates_available.len() - 5
                ),
                Style::default().fg(Color::DarkGray),
            )]));
        }
        lines.push(Line::from(""));
    }

    // Build footer spans based on what's available
    let mut footer_spans = vec![Span::raw("  ")];

    if has_app_update {
        footer_spans.push(Span::styled(
            "[Y]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
        footer_spans.push(Span::raw(" Update app  "));
    }

    if has_plugin_updates {
        footer_spans.push(Span::styled(
            "[P]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        footer_spans.push(Span::raw(" Plugins  "));
    }

    footer_spans.push(Span::styled(
        "[N]",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));
    footer_spans.push(Span::raw(" Later  "));

    if has_app_update {
        footer_spans.push(Span::styled(
            "[S]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
        footer_spans.push(Span::raw(" Skip"));
    }

    // Add footer to content
    lines.push(Line::from(footer_spans));

    let content = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);
}

fn render_plugin_upgrade_overlay(
    f: &mut Frame,
    state: &AppState,
    plugin_sub_state: &PluginUpgradeSubState,
) {
    match plugin_sub_state {
        PluginUpgradeSubState::PluginList {
            updates,
            selected_index,
        } => {
            render_plugin_update_list(f, state, updates, *selected_index);
        }
        PluginUpgradeSubState::Downloading {
            plugin_name,
            current_version,
            latest_version,
            progress,
            bytes_downloaded,
            total_bytes,
        } => {
            render_plugin_downloading(
                f,
                state,
                plugin_name,
                current_version,
                latest_version,
                *progress,
                *bytes_downloaded,
                *total_bytes,
            );
        }
        PluginUpgradeSubState::Complete {
            plugin_name,
            new_version,
            remaining_updates,
        } => {
            render_plugin_update_complete(f, state, plugin_name, new_version, remaining_updates);
        }
        PluginUpgradeSubState::Error {
            plugin_name,
            message,
            remaining_updates,
        } => {
            render_plugin_update_error(f, state, plugin_name, message, remaining_updates);
        }
    }
}

fn render_plugin_update_list(
    f: &mut Frame,
    state: &AppState,
    updates: &[crate::utils::version_check::PluginUpdateInfo],
    selected_index: usize,
) {
    let height = (updates.len() + 6).min(20) as u16;
    let area = centered_rect_absolute_height(55, height, f.area());

    let title = format!(" Plugin Updates ({}) ", updates.len());

    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));

    for (i, plugin) in updates.iter().enumerate() {
        let prefix = if i == selected_index { " > " } else { "   " };
        let style = if i == selected_index {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(vec![
            Span::styled(prefix.to_string(), style),
            Span::styled(plugin.plugin_name.clone(), style),
            Span::raw("  "),
            Span::styled(
                plugin.current_version.clone(),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" → "),
            Span::styled(
                plugin.latest_version.clone(),
                Style::default().fg(Color::Green),
            ),
        ]));
    }

    lines.push(Line::from(""));

    let content = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);

    // Footer
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Enter]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Update  "),
        Span::styled(
            "[A]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("ll  "),
        Span::styled(
            "[Esc]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Back"),
    ]));

    f.render_widget(footer, footer_area);
}

#[allow(clippy::too_many_arguments)]
fn render_plugin_downloading(
    f: &mut Frame,
    state: &AppState,
    plugin_name: &str,
    current_version: &str,
    latest_version: &str,
    progress: f64,
    bytes_downloaded: u64,
    total_bytes: Option<u64>,
) {
    // Height: 1 empty + 1 version + 1 empty + 1 progress + 1 empty + 1 footer + 2 borders = 8
    let height = 8_u16;
    let area = centered_rect_absolute_height(50, height, f.area());

    let title = format!(" Updating {} ", plugin_name);

    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  Version: "),
        Span::styled(current_version, Style::default().fg(Color::Yellow)),
        Span::raw(" → "),
        Span::styled(
            latest_version,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    let content = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);

    // Progress bar (row 4 inside the box = area.y + 1 + 3)
    let gauge_area = Rect {
        x: area.x + 2,
        y: area.y + 4,
        width: area.width - 4,
        height: 1,
    };

    let progress_label = format!(
        "{} / {}",
        format_bytes(bytes_downloaded),
        total_bytes
            .map(format_bytes)
            .unwrap_or_else(|| "???".to_string())
    );

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent((progress * 100.0) as u16)
        .label(progress_label);

    f.render_widget(gauge, gauge_area);

    // Footer (row 5 inside the box = area.y + 1 + 4)
    let footer_area = Rect {
        x: area.x + 2,
        y: area.y + 6,
        width: area.width - 4,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Esc]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Cancel"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_plugin_update_complete(
    f: &mut Frame,
    state: &AppState,
    plugin_name: &str,
    new_version: &str,
    remaining_updates: &[crate::utils::version_check::PluginUpdateInfo],
) {
    let title = " Plugin Updated! ";

    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            plugin_name,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" updated to "),
        Span::styled(
            format!("v{}", new_version),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    // Restart notice
    lines.push(Line::from(vec![Span::styled(
        "  Restart totui to use the new version.",
        Style::default().fg(Color::DarkGray),
    )]));
    lines.push(Line::from(""));

    if !remaining_updates.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            format!("  {} more plugin(s) have updates.", remaining_updates.len()),
            Style::default().fg(Color::Yellow),
        )]));
        lines.push(Line::from(""));
    }

    // Add footer to content
    let footer_spans = if !remaining_updates.is_empty() {
        vec![
            Span::raw("  "),
            Span::styled(
                "[Enter]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Continue  "),
            Span::styled(
                "[Esc]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Done"),
        ]
    } else {
        vec![
            Span::raw("  "),
            Span::styled(
                "[Enter]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Done  "),
            Span::styled(
                "[Esc]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Done"),
        ]
    };
    lines.push(Line::from(footer_spans));

    // Calculate height: content lines + 2 for borders
    let height = (lines.len() + 2) as u16;
    let area = centered_rect_absolute_height(50, height, f.area());

    let content = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);
}

fn render_plugin_update_error(
    f: &mut Frame,
    state: &AppState,
    plugin_name: &str,
    message: &str,
    remaining_updates: &[crate::utils::version_check::PluginUpdateInfo],
) {
    let title = " Plugin Update Error ";

    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  Plugin: "),
        Span::styled(
            plugin_name,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    // Wrap long error messages
    let error_prefix = "  Error: ";
    let max_error_width = 50; // Leave room for borders and padding
    let error_lines: Vec<&str> = message
        .as_bytes()
        .chunks(max_error_width)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect();

    for (i, error_line) in error_lines.iter().enumerate() {
        if i == 0 {
            lines.push(Line::from(vec![
                Span::raw(error_prefix),
                Span::styled(*error_line, Style::default().fg(Color::Red)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw("         "), // Indent continuation
                Span::styled(*error_line, Style::default().fg(Color::Red)),
            ]));
        }
    }
    lines.push(Line::from(""));

    if !remaining_updates.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            format!(
                "  {} other plugin(s) have updates.",
                remaining_updates.len()
            ),
            Style::default().fg(Color::Yellow),
        )]));
        lines.push(Line::from(""));
    }

    // Add footer to content
    let footer_spans = if !remaining_updates.is_empty() {
        vec![
            Span::raw("  "),
            Span::styled(
                "[R]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("etry  "),
            Span::styled(
                "[Enter]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Continue  "),
            Span::styled(
                "[Esc]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Done"),
        ]
    } else {
        vec![
            Span::raw("  "),
            Span::styled(
                "[R]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("etry  "),
            Span::styled(
                "[Esc]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Done"),
        ]
    };
    lines.push(Line::from(footer_spans));

    // Calculate height: content lines + 2 for borders
    let height = (lines.len() + 2) as u16;
    let area = centered_rect_absolute_height(60, height, f.area());

    let content = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);
}

fn render_upgrade_downloading(
    f: &mut Frame,
    state: &AppState,
    progress: f64,
    bytes_downloaded: u64,
    total_bytes: Option<u64>,
) {
    let area = centered_rect(50, 25, f.area());

    let current_version = env!("CARGO_PKG_VERSION");
    let new_version = state.new_version_available.as_ref().unwrap();

    // Build content lines
    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  Upgrading: "),
        Span::styled(
            format!("v{}", current_version),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" -> "),
        Span::styled(
            format!("v{}", new_version),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    let content = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Downloading Update ")
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);

    // Render progress bar
    let gauge_area = Rect {
        x: area.x + 2,
        y: area.y + 4,
        width: area.width - 4,
        height: 1,
    };

    let progress_label = format!(
        "{} / {}",
        format_bytes(bytes_downloaded),
        total_bytes
            .map(format_bytes)
            .unwrap_or_else(|| "???".to_string())
    );

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent((progress * 100.0) as u16)
        .label(progress_label);

    f.render_widget(gauge, gauge_area);

    // Render footer
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Esc]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Cancel"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_upgrade_error(f: &mut Frame, state: &AppState, message: &str) {
    let area = centered_rect(60, 30, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Download Error ")
        .style(Style::default().bg(state.theme.background).fg(Color::Red));

    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        format!("  {}", message),
        Style::default().fg(Color::Red),
    )]));
    lines.push(Line::from(""));

    let content = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });

    f.render_widget(Clear, area);
    f.render_widget(content, area);

    // Render footer
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[R]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("etry  "),
        Span::styled(
            "[Esc]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Dismiss"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_upgrade_restart_prompt(f: &mut Frame, state: &AppState) {
    let area = centered_rect(55, 35, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Update Ready ")
        .style(Style::default().bg(state.theme.background));

    let lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Download complete!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  The application will restart to complete the update.",
            Style::default().fg(state.theme.foreground),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Any unsaved changes will be lost.",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(""),
    ];

    let content = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);

    // Render footer
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Y]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("es - Restart now  "),
        Span::styled(
            "[N]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("o - Later"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_project_overlay(f: &mut Frame, state: &AppState, project_state: &ProjectSubState) {
    match project_state {
        ProjectSubState::Selecting {
            projects,
            selected_index,
        } => render_project_selecting(f, state, projects, *selected_index),
        ProjectSubState::CreateInput {
            input_buffer,
            cursor_pos,
        } => render_project_create_input(f, state, input_buffer, *cursor_pos),
        ProjectSubState::RenameInput {
            project_name,
            input_buffer,
            cursor_pos,
        } => render_project_rename_input(f, state, project_name, input_buffer, *cursor_pos),
        ProjectSubState::ConfirmDelete { project_name } => {
            render_project_confirm_delete(f, state, project_name)
        }
    }
}

fn render_project_selecting(
    f: &mut Frame,
    state: &AppState,
    projects: &[crate::project::Project],
    selected_index: usize,
) {
    let area = centered_rect(50, 50, f.area());

    let items: Vec<ListItem> = projects
        .iter()
        .enumerate()
        .map(|(i, project)| {
            let is_current = project.name == state.current_project.name;
            let is_default = project.name == DEFAULT_PROJECT_NAME;

            let marker = if is_current { "● " } else { "  " };

            let name_style = if i == selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else if is_current {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(state.theme.foreground)
            };

            let suffix = if is_default { " (default)" } else { "" };

            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Green)),
                Span::styled(format!("{}{}", project.name, suffix), name_style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Projects (Enter to switch, Esc to cancel) ")
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(list, area);

    // Render footer with options
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[n]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("ew  "),
        Span::styled(
            "[r]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("ename  "),
        Span::styled(
            "[d]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("elete"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_project_create_input(
    f: &mut Frame,
    state: &AppState,
    input_buffer: &str,
    cursor_pos: usize,
) {
    let area = centered_rect(50, 20, f.area());

    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 2,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(4),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Create New Project (Esc to cancel) ")
        .style(Style::default().bg(state.theme.background));

    f.render_widget(Clear, area);
    f.render_widget(block, area);

    // Render prompt
    let prompt_area = Rect {
        x: inner_area.x,
        y: inner_area.y,
        width: inner_area.width,
        height: 1,
    };
    let prompt = Paragraph::new("Project name:").style(Style::default().fg(state.theme.foreground));
    f.render_widget(prompt, prompt_area);

    // Render input with cursor
    let input_area = Rect {
        x: inner_area.x,
        y: inner_area.y + 1,
        width: inner_area.width,
        height: 1,
    };

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
    f.render_widget(input_paragraph, input_area);
}

fn render_project_rename_input(
    f: &mut Frame,
    state: &AppState,
    project_name: &str,
    input_buffer: &str,
    cursor_pos: usize,
) {
    let area = centered_rect(50, 25, f.area());

    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 2,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(4),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Rename '{}' (Esc to cancel) ", project_name))
        .style(Style::default().bg(state.theme.background));

    f.render_widget(Clear, area);
    f.render_widget(block, area);

    // Render prompt
    let prompt_area = Rect {
        x: inner_area.x,
        y: inner_area.y,
        width: inner_area.width,
        height: 1,
    };
    let prompt = Paragraph::new("New name:").style(Style::default().fg(state.theme.foreground));
    f.render_widget(prompt, prompt_area);

    // Render input with cursor
    let input_area = Rect {
        x: inner_area.x,
        y: inner_area.y + 1,
        width: inner_area.width,
        height: 1,
    };

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
    f.render_widget(input_paragraph, input_area);
}

fn render_project_confirm_delete(f: &mut Frame, state: &AppState, project_name: &str) {
    let area = centered_rect(50, 25, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm Delete ")
        .style(Style::default().bg(state.theme.background).fg(Color::Red));

    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        format!("  Delete project '{}'?", project_name),
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  This will delete all todos in this project.",
        Style::default().fg(state.theme.foreground),
    )]));
    lines.push(Line::from(vec![Span::styled(
        "  This action cannot be undone!",
        Style::default().fg(Color::Yellow),
    )]));
    lines.push(Line::from(""));

    let content = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);

    // Render footer with options
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Y]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("es - Delete permanently  "),
        Span::styled(
            "[N]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("o - Cancel"),
    ]));

    f.render_widget(footer, footer_area);
}

pub fn render_move_to_project_modal(frame: &mut Frame, state: &AppState) {
    let move_state = match &state.move_to_project_state {
        Some(s) => s,
        None => return,
    };

    let MoveToProjectSubState::Selecting {
        projects,
        selected_index,
        item_index,
    } = move_state;

    // Get the item being moved for display
    let item_name = state
        .todo_list
        .items
        .get(*item_index)
        .map(|i| i.content.as_str())
        .unwrap_or("(unknown)");

    // Build title with truncated item name
    let max_title_len = 40;
    let truncated_name = if item_name.len() > max_title_len {
        format!("{}...", &item_name[..max_title_len.saturating_sub(3)])
    } else {
        item_name.to_string()
    };
    let title = format!(
        " Move '{}' to (j/k to navigate, Enter to select) ",
        truncated_name
    );

    let area = centered_rect(60, 50, frame.area());

    // Clear background
    frame.render_widget(Clear, area);

    // Render project list
    let items: Vec<ListItem> = projects
        .iter()
        .enumerate()
        .map(|(i, project)| {
            let name_style = if i == *selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(state.theme.foreground)
            };
            ListItem::new(Line::from(Span::styled(&project.name, name_style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(state.theme.background)),
    );

    frame.render_widget(list, area);
}

/// Render the plugin error popup overlay.
/// Shows loading errors with plugin names and messages, plus a hint to run `totui plugin status`.
pub fn render_plugin_error_popup(f: &mut Frame, state: &AppState) {
    if !state.show_plugin_error_popup || state.pending_plugin_errors.is_empty() {
        return;
    }

    let errors = &state.pending_plugin_errors;
    let area = f.area();

    // Center popup, 80% width to accommodate longer error messages, height based on error count
    let popup_width = (area.width * 80) / 100;
    let content_lines = errors.len() * 3 + 6; // ~3 lines per error (wrapped) + header/footer
    let popup_height = (content_lines as u16 + 4).min((area.height * 70) / 100);

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height - popup_height) / 2),
            Constraint::Length(popup_height),
            Constraint::Min(0),
        ])
        .split(area);

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width - popup_width) / 2),
            Constraint::Length(popup_width),
            Constraint::Min(0),
        ])
        .split(popup_layout[1])[1];

    // Clear background
    f.render_widget(Clear, popup_area);

    // Build error text
    let mut lines = vec![
        Line::from(Span::styled(
            format!("{} plugin(s) failed to load:", errors.len()),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for error in errors {
        lines.push(Line::from(vec![
            Span::styled("  - ", Style::default().fg(Color::Red)),
            Span::styled(
                &error.plugin_name,
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(&error.message, Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Run `totui plugin status` for details",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press any key to dismiss",
        Style::default().fg(Color::Yellow),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Plugin Loading Errors ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(state.theme.background));

    f.render_widget(paragraph, popup_area);
}
