use crate::app::sync::SyncDialog;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame, dialog: &SyncDialog) {
    let area = frame.area();
    let area = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" Sync conflict — both versions changed ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(2),
            Constraint::Length(3),
        ])
        .split(inner);
    let title = dialog
        .conflict
        .local
        .as_ref()
        .or_else(|| {
            dialog
                .conflict
                .server
                .as_ref()
                .and_then(|s| s.resource.as_ref())
        })
        .map(|r| r.item.content.as_str())
        .unwrap_or("Deleted task");
    frame.render_widget(Paragraph::new(title).wrap(Wrap { trim: false }), chunks[0]);
    let field = dialog.fields.get(dialog.selected);
    let labels = if let Some(field) = field {
        format!(
            "↑/↓ field {}/{}: {} [{}] • PgUp/PgDn scroll",
            dialog.selected + 1,
            dialog.fields.len(),
            field.name,
            match field.choice {
                Some(true) => "mine",
                Some(false) => "server",
                None => "undecided",
            }
        )
    } else {
        "Values match, but the task's version changed. Choose which version to keep.".into()
    };
    frame.render_widget(Paragraph::new(labels).wrap(Wrap { trim: false }), chunks[1]);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);
    for (index, title) in ["Mine", "Server"].into_iter().enumerate() {
        let value = field
            .map(|f| {
                if index == 0 {
                    f.mine.as_str()
                } else {
                    f.server.as_str()
                }
            })
            .unwrap_or("(no differing fields)");
        frame.render_widget(
            Paragraph::new(value)
                .scroll((dialog.scroll, 0))
                .wrap(Wrap { trim: false })
                .block(Block::default().title(title).borders(Borders::ALL)),
            panes[index],
        );
    }
    let hints = if dialog.combining {
        "l/s choose this field • Enter save combination • Esc later"
    } else {
        "l keep mine • s keep server • m combine fields • Esc later (F6 reopens)"
    };
    frame.render_widget(
        Paragraph::new(vec![Line::from(hints), Line::from(dialog.message.as_str())])
            .style(Style::default().fg(Color::Yellow))
            .wrap(Wrap { trim: false }),
        chunks[3],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    use to_tui::remote::{
        cache::Conflict,
        protocol::{TaskResource, TaskVersion},
    };
    use to_tui::todo::TodoItem;

    #[test]
    fn test_conflict_dialog_displays_both_versions_and_controls() {
        let mine = TaskResource {
            project: "default".into(),
            date: chrono::Local::now().date_naive(),
            position: 0,
            item: TodoItem::new("My title".into(), 0),
        };
        let mut server = mine.clone();
        server.item.content = "Server title".into();
        let dialog = SyncDialog::new(Conflict {
            id: mine.item.id,
            base: None,
            local: Some(mine),
            server: Some(TaskVersion {
                id: server.item.id,
                etag: "v2".into(),
                resource: Some(server),
            }),
        });
        for (width, height) in [(80, 24), (120, 40)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|f| render(f, &dialog)).unwrap();
            let visible: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            for expected in [
                "My title",
                "Server title",
                "l keep mine",
                "s keep server",
                "m combine fields",
            ] {
                assert!(visible.contains(expected), "Missing {expected}");
            }
        }
    }
}
