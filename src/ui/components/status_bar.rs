use crate::app::AppState;
use crate::app::mode::Mode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FooterLink {
    Web,
    Github,
    Upgrade,
}

fn content(state: &AppState) -> [String; 5] {
    let readonly = if state.is_readonly() {
        " [READONLY]"
    } else {
        ""
    };
    let unsaved = if state.unsaved_changes {
        " [unsaved]"
    } else {
        ""
    };
    let day = if state.viewing_date == state.today {
        "today"
    } else {
        "archived"
    };
    let remote_label = to_tui::remote::active()
        .map(|client| format!("[{}] ", client.url()))
        .unwrap_or_default();
    let project = if state.current_project.name != crate::project::DEFAULT_PROJECT_NAME {
        format!("{remote_label}[{}] ", state.current_project.name)
    } else {
        remote_label
    };
    let sync = to_tui::remote::active()
        .and_then(|c| c.cached())
        .map(|cache| {
            let status = cache.status();
            if status.conflicts > 0 {
                format!(" | {} conflicts (F6)", status.conflicts)
            } else if status.error.is_some() {
                format!(" | offline/error, {} queued (F6)", status.pending)
            } else if status.pending > 0 {
                format!(" | {} queued", status.pending)
            } else {
                " | synced".into()
            }
        })
        .unwrap_or_default();
    let info = match &state.status_message {
        Some((message, time)) if time.elapsed().as_secs() <= 3 => format!(" {message} "),
        _ => format!(
            " {project}{} | {} ({day}) | {} items{readonly}{unsaved}{sync}",
            state.mode,
            state.viewing_date.format("%Y-%m-%d"),
            state.todo_list.items.len()
        ),
    };
    let hints = if state.is_readonly() {
        " < prev  > next  T today "
    } else {
        " ? help  q quit "
    };
    let shortcut = state
        .keybindings
        .navigate_bindings_for(crate::keybindings::Action::OpenWebManager)
        .filter(|bindings| !bindings.is_empty())
        .map(|bindings| bindings.join(" / "))
        .unwrap_or_else(|| "click".into());
    let version = match &state.new_version_available {
        Some(version) => format!(" v{VERSION} → v{version} "),
        None => format!(" v{VERSION} "),
    };
    let web = if to_tui::remote::active().is_some() {
        format!(" {shortcut} web-ui ")
    } else {
        format!(" {shortcut} web-ui ({}) ", state.web.label())
    };
    [info, hints.into(), web, " 🔗 ".into(), version]
}

fn sections(area: Rect, content: &[String; 5]) -> [Rect; 5] {
    use unicode_width::UnicodeWidthStr;

    let mut widths = content
        .each_ref()
        .map(|text| text.width().min(u16::MAX as usize) as u16);
    let available = area.width as usize;
    if widths[1..]
        .iter()
        .map(|width| *width as usize)
        .sum::<usize>()
        > available
    {
        widths[4] = 0;
    }
    if widths[1..]
        .iter()
        .map(|width| *width as usize)
        .sum::<usize>()
        > available
    {
        widths[3] = 0;
    }
    widths[2] = widths[2].min(area.width);
    widths[1] = widths[1].min(area.width.saturating_sub(widths[2] + widths[3] + widths[4]));
    let reserved = widths[1..]
        .iter()
        .map(|width| *width as usize)
        .sum::<usize>();
    widths[0] = widths[0].min(available.saturating_sub(reserved) as u16);
    let mut x = area.x;
    std::array::from_fn(|index| {
        if index == 3 {
            x = area.right().saturating_sub(widths[3] + widths[4]);
        }
        let rect = Rect::new(x, area.y, widths[index], area.height);
        x += widths[index];
        rect
    })
}

pub fn link_at(state: &AppState, row: usize, col: usize) -> Option<FooterLink> {
    if state.show_help
        || !matches!(state.mode, Mode::Navigate | Mode::Edit | Mode::Visual)
        || row != state.terminal_height.saturating_sub(1) as usize
    {
        return None;
    }
    let area = Rect::new(0, row as u16, state.terminal_width, 1);
    let regions = sections(area, &content(state));
    let contains = |index: usize| {
        let region = regions[index];
        col >= region.x as usize && col < region.right() as usize
    };
    if state.mode == Mode::Navigate && contains(2) {
        Some(FooterLink::Web)
    } else if contains(3) {
        Some(FooterLink::Github)
    } else if state.new_version_available.is_some() && contains(4) {
        Some(FooterLink::Upgrade)
    } else {
        None
    }
}

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    if state.mode == Mode::ConfirmDelete {
        render_confirm_delete(f, state, area);
        return;
    }
    let mut style = Style::default()
        .fg(state.theme.status_bar_fg)
        .bg(state.theme.status_bar_bg);
    if state.is_readonly() {
        style = style.add_modifier(Modifier::BOLD);
    }
    f.render_widget(Paragraph::new("").style(style), area);
    let content = content(state);
    for (index, (text, region)) in content.iter().zip(sections(area, &content)).enumerate() {
        if index == 3
            && region.width >= 4
            && let Some(icon) = &state.github_icon
        {
            f.render_widget(icon, Rect::new(region.x + 1, region.y, 2, 1));
        } else {
            f.render_widget(Paragraph::new(text.as_str()).style(style), region);
        }
    }
}

fn render_confirm_delete(f: &mut Frame, state: &AppState, area: Rect) {
    let subtask_count = state.pending_delete_subtask_count.unwrap_or(0);
    let prompt = format!(
        " Delete task and its {} subtask{}? (Y/n) ",
        subtask_count,
        if subtask_count == 1 { "" } else { "s" }
    );

    let style = Style::default()
        .fg(ratatui::style::Color::White)
        .bg(ratatui::style::Color::Rgb(180, 100, 0))
        .add_modifier(Modifier::BOLD);

    let padding = area.width.saturating_sub(prompt.len() as u16);
    let status_line = format!("{}{:padding$}", prompt, "", padding = padding as usize);

    let status = Paragraph::new(Line::from(vec![Span::styled(status_line, style)]));
    f.render_widget(status, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn test_footer_layout_keeps_controls_visible_with_wide_characters() {
        let content = [
            " 工作 project ".into(),
            " ? help  q quit ".into(),
            " w web-ui (running) ".into(),
            " 🔗 ".into(),
            " v0.6.0-dev-long-version ".into(),
        ];
        for width in [40, 80, 100, 160] {
            let regions = sections(Rect::new(0, 0, width, 1), &content);
            assert_eq!(regions[2].width as usize, content[2].width());
            for pair in regions.windows(2) {
                assert!(pair[0].right() <= pair[1].x);
            }
            assert!(regions[4].right() <= width);
            if regions[3].width > 0 {
                assert_eq!(regions[3].width as usize, content[3].width());
            }
        }
    }
}
