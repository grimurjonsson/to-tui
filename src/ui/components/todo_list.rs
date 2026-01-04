use crate::app::{AppState, Mode};
use crate::todo::TodoState;
use crate::utils::unicode::{after_first_char, first_char_as_str};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};
use std::collections::HashSet;
use unicode_width::UnicodeWidthStr;

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let mut items: Vec<ListItem> = Vec::new();
    let hidden_indices = build_hidden_indices(state);
    let available_width = area.width.saturating_sub(2) as usize;

    for (idx, item) in state.todo_list.items.iter().enumerate() {
        if hidden_indices.contains(&idx) {
            continue;
        }

        let indent = "  ".repeat(item.indent_level);
        let has_children = state.todo_list.has_children(idx);

        let fold_icon = if has_children {
            if item.collapsed { "▶ " } else { "▼ " }
        } else {
            "  "
        };

        let checkbox = format!("{}", item.state);

        let due_date_str = item
            .due_date
            .map(|d| format!(" [{}]", d.format("%Y-%m-%d")))
            .unwrap_or_default();

        let collapse_indicator = if item.collapsed {
            let (completed, total) = state.todo_list.count_children_stats(idx);
            format!(" ({completed}/{total})")
        } else {
            String::new()
        };

        let prefix = format!("{indent}{fold_icon}");
        let prefix_width = prefix.width();
        let checkbox_with_space = format!("{checkbox} ");
        let checkbox_width = checkbox_with_space.width();
        let content_with_extras = format!("{}{}{}", item.content, due_date_str, collapse_indicator);

        let is_cursor = idx == state.cursor_position && state.mode == Mode::Navigate;
        let is_visual_cursor = idx == state.cursor_position && state.mode == Mode::Visual;
        let is_in_selection = state.is_selected(idx) && state.mode == Mode::Visual;

        let prefix_style = if is_cursor || is_visual_cursor {
            Style::default()
                .fg(state.theme.cursor)
                .add_modifier(Modifier::REVERSED)
        } else if is_in_selection {
            Style::default()
                .bg(ratatui::style::Color::DarkGray)
                .fg(state.theme.foreground)
        } else {
            Style::default().fg(state.theme.foreground)
        };

        let content_style = if is_cursor || is_visual_cursor {
            Style::default()
                .fg(state.theme.cursor)
                .add_modifier(Modifier::REVERSED)
        } else if is_in_selection {
            Style::default()
                .bg(ratatui::style::Color::DarkGray)
                .fg(state.theme.foreground)
        } else {
            match item.state {
                TodoState::Checked => Style::default().fg(ratatui::style::Color::DarkGray),
                TodoState::Question => Style::default().fg(state.theme.question),
                TodoState::Exclamation => Style::default().fg(state.theme.exclamation),
                _ => Style::default().fg(state.theme.foreground),
            }
        };

        let content_max_width = available_width.saturating_sub(prefix_width + checkbox_width);

        let is_editing_this_item =
            state.mode == Mode::Edit && !state.is_creating_new_item && idx == state.cursor_position;

        if is_editing_this_item {
            let edit_lines =
                build_wrapped_edit_lines_for_existing(state, available_width, item.indent_level);
            items.push(ListItem::new(edit_lines));
        } else {
            let wrapped_lines = wrap_text(&content_with_extras, content_max_width);
            let continuation_indent = " ".repeat(prefix_width + checkbox_width);

            let mut lines: Vec<Line> = Vec::new();
            for (i, line_text) in wrapped_lines.iter().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(prefix.clone(), prefix_style),
                        Span::styled(checkbox_with_space.clone(), content_style),
                        Span::styled(line_text.clone(), content_style),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(continuation_indent.clone(), prefix_style),
                        Span::styled(line_text.clone(), content_style),
                    ]));
                }
            }

            items.push(ListItem::new(lines));
        }

        if !item.collapsed {
            if let Some(ref desc) = item.description {
                let desc_prefix = format!("{}    ", "  ".repeat(item.indent_level));
                let desc_prefix_width = desc_prefix.width();
                let desc_max_width = available_width.saturating_sub(desc_prefix_width);
                let desc_wrapped = wrap_text(desc, desc_max_width);
                let desc_continuation = " ".repeat(desc_prefix_width);
                let desc_style = Style::default()
                    .fg(ratatui::style::Color::DarkGray)
                    .add_modifier(Modifier::ITALIC);

                let mut desc_lines: Vec<Line> = Vec::new();
                for (i, line_text) in desc_wrapped.iter().enumerate() {
                    if i == 0 {
                        desc_lines.push(Line::from(vec![
                            Span::styled(desc_prefix.clone(), desc_style),
                            Span::styled(line_text.clone(), desc_style),
                        ]));
                    } else {
                        desc_lines.push(Line::from(vec![
                            Span::styled(desc_continuation.clone(), desc_style),
                            Span::styled(line_text.clone(), desc_style),
                        ]));
                    }
                }
                items.push(ListItem::new(desc_lines));
            }
        }

        if state.is_creating_new_item && state.mode == Mode::Edit && idx == state.cursor_position {
            let new_item_lines = build_wrapped_edit_lines(state, available_width);
            items.push(ListItem::new(new_item_lines));
        }
    }

    if state.todo_list.items.is_empty() {
        if state.is_creating_new_item && state.mode == Mode::Edit {
            let new_item_lines = build_wrapped_edit_lines(state, available_width);
            items.push(ListItem::new(new_item_lines));
        } else {
            // Show helpful message when list is empty
            items.push(ListItem::new(Line::from(Span::styled(
                "",
                Style::default(),
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "  No todos for today!",
                Style::default().fg(state.theme.foreground),
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "",
                Style::default(),
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "  Press 'n' to create a new todo",
                Style::default().fg(state.theme.foreground),
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "  Press '?' for help",
                Style::default().fg(state.theme.foreground),
            ))));
        }
    }

    let title = format!(" Todo List - {} ", state.todo_list.date.format("%B %d, %Y"));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(list, area);
}

fn build_wrapped_edit_lines(state: &AppState, available_width: usize) -> Vec<Line<'static>> {
    build_wrapped_edit_lines_with_indent(state, available_width, state.pending_indent_level)
}

fn build_wrapped_edit_lines_for_existing(
    state: &AppState,
    available_width: usize,
    indent_level: usize,
) -> Vec<Line<'static>> {
    build_wrapped_edit_lines_with_indent(state, available_width, indent_level)
}

fn build_wrapped_edit_lines_with_indent(
    state: &AppState,
    available_width: usize,
    indent_level: usize,
) -> Vec<Line<'static>> {
    let indent = "  ".repeat(indent_level);
    let fold_icon_space = "  ";
    let prefix = format!("{indent}{fold_icon_space}[ ] ");
    let prefix_width = prefix.width();
    let content_max_width = available_width.saturating_sub(prefix_width);

    let edit_wrapped = wrap_text(&state.edit_buffer, content_max_width);
    let edit_row_count = edit_wrapped.len();
    let cursor_line =
        find_cursor_line(&state.edit_buffer, state.edit_cursor_pos, content_max_width);

    let mut lines: Vec<Line<'static>> = Vec::new();

    for (line_idx, line_text) in edit_wrapped.iter().enumerate() {
        let line_prefix = if line_idx == 0 {
            prefix.clone()
        } else {
            " ".repeat(prefix_width)
        };

        if cursor_line == line_idx {
            let cursor_pos_in_line = find_cursor_pos_in_wrapped_line(
                &state.edit_buffer,
                state.edit_cursor_pos,
                content_max_width,
                line_idx,
            );
            let before_cursor = &line_text[..cursor_pos_in_line.min(line_text.len())];
            let after_cursor = &line_text[cursor_pos_in_line.min(line_text.len())..];

            let mut spans: Vec<Span<'static>> = vec![
                Span::styled(line_prefix, Style::default()),
                Span::styled(before_cursor.to_string(), Style::default()),
            ];

            if after_cursor.is_empty() && line_idx == edit_row_count - 1 {
                spans.push(Span::styled(
                    "█",
                    Style::default()
                        .fg(ratatui::style::Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            } else if !after_cursor.is_empty() {
                spans.push(Span::styled(
                    first_char_as_str(after_cursor).to_string(),
                    Style::default()
                        .bg(ratatui::style::Color::Yellow)
                        .fg(ratatui::style::Color::Black)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    after_first_char(after_cursor).to_string(),
                    Style::default(),
                ));
            } else {
                spans.push(Span::styled(
                    "█",
                    Style::default()
                        .fg(ratatui::style::Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            lines.push(Line::from(spans));
        } else {
            let spans: Vec<Span<'static>> = vec![
                Span::styled(line_prefix, Style::default()),
                Span::styled(line_text.clone(), Style::default()),
            ];
            lines.push(Line::from(spans));
        }
    }

    lines
}

fn find_cursor_line(text: &str, cursor_pos: usize, max_width: usize) -> usize {
    if max_width == 0 || text.is_empty() {
        return 0;
    }

    let mut current_line = 0;
    let mut line_start_byte = 0;
    let mut current_width = 0;
    let mut last_space_byte = None;

    for (byte_idx, c) in text.char_indices() {
        if byte_idx >= cursor_pos {
            return current_line;
        }

        let char_width = c.to_string().width();

        if c == ' ' {
            last_space_byte = Some(byte_idx);
        }

        if current_width + char_width > max_width && current_width > 0 {
            if let Some(space_byte) = last_space_byte {
                if space_byte > line_start_byte {
                    if cursor_pos <= space_byte {
                        return current_line;
                    }
                    current_line += 1;
                    line_start_byte = space_byte + 1;
                    current_width = text[line_start_byte..=byte_idx].width();
                    last_space_byte = None;
                    continue;
                }
            }
            current_line += 1;
            line_start_byte = byte_idx;
            current_width = char_width;
            last_space_byte = None;
        } else {
            current_width += char_width;
        }
    }

    current_line
}

fn find_cursor_pos_in_wrapped_line(
    text: &str,
    cursor_pos: usize,
    max_width: usize,
    target_line: usize,
) -> usize {
    let wrapped = wrap_text(text, max_width);
    if target_line >= wrapped.len() {
        return 0;
    }

    let mut byte_offset = 0;
    for (line_idx, line) in wrapped.iter().enumerate() {
        if line_idx == target_line {
            break;
        }
        byte_offset += line.len();
        while byte_offset < text.len() {
            let next_char = text[byte_offset..].chars().next();
            if next_char == Some(' ') {
                byte_offset += 1;
            } else {
                break;
            }
        }
    }

    if cursor_pos < byte_offset {
        return 0;
    }

    let pos_in_line = cursor_pos - byte_offset;
    pos_in_line.min(wrapped[target_line].len())
}

fn build_hidden_indices(state: &AppState) -> HashSet<usize> {
    let mut hidden = HashSet::new();
    let items = &state.todo_list.items;

    let mut i = 0;
    while i < items.len() {
        if items[i].collapsed {
            let base_indent = items[i].indent_level;
            let mut j = i + 1;
            while j < items.len() && items[j].indent_level > base_indent {
                hidden.insert(j);
                j += 1;
            }
            i = j;
        } else {
            i += 1;
        }
    }

    hidden
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_width = word.width();

        if current_line.is_empty() {
            current_line = word.to_string();
            current_width = word_width;
        } else if current_width + 1 + word_width <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
            current_width += 1 + word_width;
        } else {
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_width;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}
