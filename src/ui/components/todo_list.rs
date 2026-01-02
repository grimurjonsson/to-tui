use crate::app::{AppState, Mode};
use crate::todo::TodoState;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let mut items: Vec<ListItem> = Vec::new();

    // Build the list of items, potentially inserting a placeholder for new item
    for (idx, item) in state.todo_list.items.iter().enumerate() {
        let indent = "  ".repeat(item.indent_level);
        let checkbox = format!("{}", item.state);
        let content = format!("{}{} {}", indent, checkbox, item.content);

        let style = if idx == state.cursor_position && state.mode != Mode::Edit {
            // Highlight in Navigate mode, but preserve strikethrough for completed items
            let mut style = Style::default()
                .fg(state.theme.cursor)
                .add_modifier(Modifier::REVERSED);

            if item.state == TodoState::Checked {
                style = style.add_modifier(Modifier::CROSSED_OUT);
            }

            style
        } else {
            match item.state {
                TodoState::Checked => Style::default()
                    .fg(state.theme.foreground)
                    .add_modifier(Modifier::CROSSED_OUT),
                TodoState::Question => Style::default().fg(state.theme.question),
                TodoState::Exclamation => Style::default().fg(state.theme.exclamation),
                _ => Style::default().fg(state.theme.foreground),
            }
        };

        items.push(ListItem::new(Line::from(Span::styled(content, style))));

        // If creating a new item and we just rendered the current item, insert placeholder
        if state.is_creating_new_item && state.mode == Mode::Edit && idx == state.cursor_position {
            let indent = "  ".repeat(state.pending_indent_level);
            let prefix = format!("{}[ ] ", indent);

            // Split text at cursor position to show cursor
            let before_cursor = &state.edit_buffer[..state.edit_cursor_pos];
            let after_cursor = &state.edit_buffer[state.edit_cursor_pos..];

            // Build line with visible cursor - normal text with bright cursor
            let mut spans = vec![
                Span::styled(prefix, Style::default()),
                Span::styled(before_cursor, Style::default()),
            ];

            // Cursor: bright block or inverted character
            if after_cursor.is_empty() {
                // At end: show bright yellow/green block
                spans.push(Span::styled(
                    "█",
                    Style::default()
                        .fg(ratatui::style::Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                ));
            } else {
                // In middle: invert the character with bright colors
                spans.push(Span::styled(
                    &after_cursor[..1],
                    Style::default()
                        .bg(ratatui::style::Color::Yellow)
                        .fg(ratatui::style::Color::Black)
                        .add_modifier(Modifier::BOLD)
                ));
                spans.push(Span::styled(
                    &after_cursor[1..],
                    Style::default()
                ));
            }

            items.push(ListItem::new(Line::from(spans)));
        }
    }

    // Handle empty list case
    if state.todo_list.items.is_empty() {
        if state.is_creating_new_item && state.mode == Mode::Edit {
            // Creating first item with visible cursor
            let indent = "  ".repeat(state.pending_indent_level);
            let prefix = format!("{}[ ] ", indent);
            let before_cursor = &state.edit_buffer[..state.edit_cursor_pos];
            let after_cursor = &state.edit_buffer[state.edit_cursor_pos..];

            // Build line with visible cursor - normal text with bright cursor
            let mut spans = vec![
                Span::styled(prefix, Style::default()),
                Span::styled(before_cursor, Style::default()),
            ];

            // Cursor: bright block or inverted character
            if after_cursor.is_empty() {
                // At end: show bright yellow/green block
                spans.push(Span::styled(
                    "█",
                    Style::default()
                        .fg(ratatui::style::Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                ));
            } else {
                // In middle: invert the character with bright colors
                spans.push(Span::styled(
                    &after_cursor[..1],
                    Style::default()
                        .bg(ratatui::style::Color::Yellow)
                        .fg(ratatui::style::Color::Black)
                        .add_modifier(Modifier::BOLD)
                ));
                spans.push(Span::styled(
                    &after_cursor[1..],
                    Style::default()
                ));
            }

            items.push(ListItem::new(Line::from(spans)));
        } else {
            // Show helpful message when list is empty
            items.push(ListItem::new(Line::from(Span::styled(
                "",
                Style::default()
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "  No todos for today!",
                Style::default().fg(state.theme.foreground)
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "",
                Style::default()
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "  Press 'n' to create a new todo",
                Style::default().fg(state.theme.foreground)
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "  Press '?' for help",
                Style::default().fg(state.theme.foreground)
            ))));
        }
    }

    let title = format!(
        " Todo List - {} ",
        state.todo_list.date.format("%B %d, %Y")
    );

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(list, area);

    // Render edit mode input if active (but not when creating new item, as it's rendered inline)
    if state.mode == Mode::Edit && !state.is_creating_new_item {
        render_edit_input(f, state, area);
    }
}

fn render_edit_input(f: &mut Frame, state: &AppState, area: Rect) {
    // Calculate position for edit input (at cursor position)
    let y_offset = state.cursor_position.saturating_sub(state.scroll_offset) as u16 + 1; // +1 for border

    if y_offset >= area.height - 1 {
        return; // Off screen
    }

    let edit_area = Rect {
        x: area.x + 1,
        y: area.y + y_offset,
        width: area.width.saturating_sub(2),
        height: 1,
    };

    let indent_level = if state.cursor_position < state.todo_list.items.len() {
        state.todo_list.items[state.cursor_position].indent_level
    } else {
        0
    };

    let indent = "  ".repeat(indent_level);
    let prefix = format!("{}[ ] ", indent);

    // Split text at cursor position to show cursor
    let before_cursor = &state.edit_buffer[..state.edit_cursor_pos];
    let after_cursor = &state.edit_buffer[state.edit_cursor_pos..];

    // Build line with visible cursor - normal text with bright cursor
    let mut spans = vec![
        Span::styled(prefix, Style::default()),
        Span::styled(before_cursor, Style::default()),
    ];

    // Cursor: bright block or inverted character
    if after_cursor.is_empty() {
        // At end: show bright yellow block
        spans.push(Span::styled(
            "█",
            Style::default()
                .fg(ratatui::style::Color::Yellow)
                .add_modifier(Modifier::BOLD)
        ));
    } else {
        // In middle: invert the character with bright yellow
        spans.push(Span::styled(
            &after_cursor[..1],
            Style::default()
                .bg(ratatui::style::Color::Yellow)
                .fg(ratatui::style::Color::Black)
                .add_modifier(Modifier::BOLD)
        ));
        spans.push(Span::styled(
            &after_cursor[1..],
            Style::default()
        ));
    }

    let edit_line = Line::from(spans);

    f.render_widget(edit_line, edit_area);
}
