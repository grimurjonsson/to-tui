use crate::app::{AppState, Mode};
use crate::todo::TodoState;
use crate::ui::theme::Theme;
use crate::utils::unicode::{after_first_char, first_char_as_str};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};
use unicode_width::UnicodeWidthStr;

/// Compute the style for a todo item based on its state and cursor position
fn compute_item_style(
    state: TodoState,
    theme: &Theme,
    is_cursor: bool,
    is_in_selection: bool,
) -> Style {
    if is_cursor {
        let cursor_color = match state {
            TodoState::Checked => Color::DarkGray,
            TodoState::Question => theme.question,
            TodoState::Exclamation => theme.exclamation,
            TodoState::InProgress => theme.in_progress,
            _ => Color::White,
        };
        Style::default()
            .fg(cursor_color)
            .add_modifier(Modifier::REVERSED)
    } else if is_in_selection {
        Style::default()
            .bg(Color::DarkGray)
            .fg(theme.foreground)
    } else {
        match state {
            TodoState::Checked => Style::default().fg(Color::DarkGray),
            TodoState::Question => Style::default().fg(theme.question),
            TodoState::Exclamation => Style::default().fg(theme.exclamation),
            TodoState::InProgress => Style::default().fg(theme.in_progress),
            _ => Style::default().fg(theme.foreground),
        }
    }
}

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let mut items: Vec<ListItem> = Vec::new();
    let hidden_indices = state.todo_list.build_hidden_indices();
    let available_width = area.width.saturating_sub(2) as usize;

    for (idx, item) in state.todo_list.items.iter().enumerate() {
        if hidden_indices.contains(&idx) {
            continue;
        }

        let indent = "  ".repeat(item.indent_level);
        let has_children = state.todo_list.has_children(idx);

        let has_description = item.description.is_some();
        let is_collapsible = has_children || has_description;

        let fold_icon = if is_collapsible {
            if item.collapsed {
                "▶ "
            } else {
                "▼ "
            }
        } else {
            "  "
        };

        let checkbox = if item.state == TodoState::InProgress {
            format!("[{}]", state.get_spinner_char())
        } else if item.state == TodoState::Empty && has_children {
            let (completed, _) = state.todo_list.count_children_stats(idx);
            if completed > 0 {
                "[-]".to_string()
            } else {
                format!("{}", item.state)
            }
        } else {
            format!("{}", item.state)
        };

        let due_date_str = item
            .due_date
            .map(|d| format!(" [{}]", d.format("%Y-%m-%d")))
            .unwrap_or_default();

        let collapse_indicator = if item.collapsed && has_children {
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

        // For prefix, we use foreground color when not highlighted (cursor/selection)
        let prefix_style = compute_item_style(
            TodoState::Empty, // Use Empty to get foreground color for prefix
            &state.theme,
            is_cursor || is_visual_cursor,
            is_in_selection,
        );

        let content_style = compute_item_style(
            item.state,
            &state.theme,
            is_cursor || is_visual_cursor,
            is_in_selection,
        );

        let content_max_width = available_width.saturating_sub(prefix_width + checkbox_width);

        let is_editing_this_item =
            state.mode == Mode::Edit && !state.is_creating_new_item && idx == state.cursor_position;

        let should_show_new_item_above = state.is_creating_new_item
            && state.mode == Mode::Edit
            && idx == state.cursor_position
            && state.insert_above;

        if should_show_new_item_above {
            let new_item_lines = build_wrapped_edit_lines(state, available_width);
            items.push(ListItem::new(new_item_lines));
        }

        if is_editing_this_item {
            let edit_lines =
                build_wrapped_edit_lines_for_existing(state, available_width, item.indent_level);
            items.push(ListItem::new(edit_lines));
        } else {
            let should_truncate = item.collapsed && has_description;

            if should_truncate {
                let content_with_due = format!("{}{}", item.content, due_date_str);
                let indicator_width = collapse_indicator.width();
                let available_for_content = content_max_width.saturating_sub(indicator_width);
                let truncated_content =
                    truncate_with_ellipsis(&content_with_due, available_for_content);
                let display_text = format!("{truncated_content}{collapse_indicator}");

                let lines = vec![Line::from(vec![
                    Span::styled(prefix.clone(), prefix_style),
                    Span::styled(checkbox_with_space.clone(), content_style),
                    Span::styled(display_text, content_style),
                ])];
                items.push(ListItem::new(lines));
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
        }

        if !item.collapsed
            && let Some(ref desc) = item.description {
                let base_indent = "  ".repeat(item.indent_level);
                let border_color = ratatui::style::Color::Rgb(100, 100, 120);
                let text_color = ratatui::style::Color::Rgb(180, 180, 190);

                let top_left = "╭";
                let top_right = "╮";
                let bottom_left = "╰";
                let bottom_right = "╯";
                let horizontal = "─";
                let vertical = "│";

                let box_indent = format!("{base_indent}    ");
                let box_indent_width = box_indent.width();
                let inner_width = available_width.saturating_sub(box_indent_width + 4);
                let desc_wrapped = wrap_text(desc, inner_width);

                let border_width = inner_width + 2;
                let top_border = format!(
                    "{}{}{}",
                    top_left,
                    horizontal.repeat(border_width),
                    top_right
                );
                let bottom_border = format!(
                    "{}{}{}",
                    bottom_left,
                    horizontal.repeat(border_width),
                    bottom_right
                );

                let border_style = Style::default().fg(border_color);
                let text_style = Style::default().fg(text_color);

                let mut desc_lines: Vec<Line> = Vec::new();

                desc_lines.push(Line::from(vec![
                    Span::styled(box_indent.clone(), Style::default()),
                    Span::styled(top_border, border_style),
                ]));

                for line_text in desc_wrapped.iter() {
                    let padding = inner_width.saturating_sub(line_text.width());
                    let padded_text = format!("{}{}", line_text, " ".repeat(padding));
                    desc_lines.push(Line::from(vec![
                        Span::styled(box_indent.clone(), Style::default()),
                        Span::styled(format!("{vertical} "), border_style),
                        Span::styled(padded_text, text_style),
                        Span::styled(format!(" {vertical}"), border_style),
                    ]));
                }

                desc_lines.push(Line::from(vec![
                    Span::styled(box_indent.clone(), Style::default()),
                    Span::styled(bottom_border, border_style),
                ]));

                items.push(ListItem::new(desc_lines));
            }

        let should_show_new_item_below = state.is_creating_new_item
            && state.mode == Mode::Edit
            && idx == state.cursor_position
            && !state.insert_above;

        if should_show_new_item_below {
            let new_item_lines = build_wrapped_edit_lines(state, available_width);
            items.push(ListItem::new(new_item_lines));
        }
    }

    if state.todo_list.items.is_empty() {
        if state.is_creating_new_item && state.mode == Mode::Edit {
            let new_item_lines = build_wrapped_edit_lines(state, available_width);
            items.push(ListItem::new(new_item_lines));
        } else if state.is_readonly() {
            items.push(ListItem::new(Line::from(Span::styled(
                "",
                Style::default(),
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "  No archived todos for this date",
                Style::default().fg(state.theme.foreground),
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "",
                Style::default(),
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "  Press '>' for next day, '<' for previous day",
                Style::default().fg(state.theme.foreground),
            ))));
            items.push(ListItem::new(Line::from(Span::styled(
                "  Press 'T' to go back to today",
                Style::default().fg(state.theme.foreground),
            ))));
        } else {
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

    let title_suffix = if state.is_readonly() {
        " (Archived)"
    } else {
        ""
    };
    let title = format!(
        " Todo List - {}{} ",
        state.viewing_date.format("%B %d, %Y"),
        title_suffix
    );

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

    let edit_wrapped = wrap_text_preserving_trailing(&state.edit_buffer, content_max_width);
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
            let cursor_pos_in_line = find_cursor_pos_in_wrapped_line_preserving(
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
            if let Some(space_byte) = last_space_byte
                && space_byte > line_start_byte {
                    if cursor_pos <= space_byte {
                        return current_line;
                    }
                    current_line += 1;
                    line_start_byte = space_byte + 1;
                    current_width = text[line_start_byte..=byte_idx].width();
                    last_space_byte = None;
                    continue;
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

fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let ellipsis = "…";
    let ellipsis_width = 1;

    let mut result = String::new();
    let mut current_width = 0;

    for c in text.chars() {
        let char_width = c.to_string().width();

        if current_width + char_width + ellipsis_width > max_width {
            result.push_str(ellipsis);
            return result;
        }

        result.push(c);
        current_width += char_width;
    }

    result
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();

    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        let mut current_width = 0;

        for word in paragraph.split_whitespace() {
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
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn wrap_text_preserving_trailing(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let trailing_spaces = text.len() - text.trim_end().len();
    let mut lines = wrap_text(text, max_width);

    if trailing_spaces > 0 && !lines.is_empty() {
        let last_idx = lines.len() - 1;
        lines[last_idx].push_str(&" ".repeat(trailing_spaces));
    }

    lines
}

fn find_cursor_pos_in_wrapped_line_preserving(
    text: &str,
    cursor_pos: usize,
    max_width: usize,
    target_line: usize,
) -> usize {
    let wrapped = wrap_text_preserving_trailing(text, max_width);
    if target_line >= wrapped.len() {
        return 0;
    }

    let mut byte_offset = 0;
    for (line_idx, line) in wrapped.iter().enumerate() {
        if line_idx == target_line {
            break;
        }
        byte_offset += line.trim_end().len();
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
