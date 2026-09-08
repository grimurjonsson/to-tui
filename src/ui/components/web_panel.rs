use crate::app::AppState;
use crate::app::web::WebAction;
use crate::web_process::WebStatus;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame, state: &AppState) {
    let screen = frame.area();
    let width = screen.width.saturating_sub(2).min(76);
    let height = screen.height.saturating_sub(2).min(20);
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    );
    let light = state.theme.background == Color::White;
    let green = if light {
        Color::Green
    } else {
        Color::LightGreen
    };
    let muted = if light { Color::DarkGray } else { Color::Gray };
    let accent = state.theme.in_progress;
    let status_color = match state.web.label() {
        "running" => green,
        "stopped" => muted,
        "error" => state.theme.priority_p0,
        _ => state.theme.priority_p1,
    };
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(muted)),
            Span::styled(
                state.web.label(),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::styled(
            "↑/↓ select   Enter run   Esc close",
            Style::default().fg(muted),
        ),
        Line::from(""),
    ];
    for action in WebAction::ALL {
        let color = match action {
            WebAction::Start => green,
            WebAction::Stop => state.theme.priority_p0,
            WebAction::Restart => state.theme.priority_p1,
            WebAction::Open => accent,
        };
        let selected = state.web.selected_action() == action;
        let style = if selected {
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };
        lines.push(Line::styled(
            format!(" {} {} ", if selected { "▶" } else { " " }, action.title()),
            style,
        ));
    }
    lines.push(Line::from(""));
    match &state.web.status {
        Some(WebStatus::Running { url, legacy }) => {
            lines.push(Line::styled(url.clone(), Style::default().fg(accent)));
            if *legacy {
                lines.push(Line::styled(
                    "Existing API daemon; restart to enable managed web logs.",
                    Style::default().fg(state.theme.priority_p1),
                ));
            }
        }
        Some(WebStatus::External) => {
            lines.push(Line::styled(
                format!(
                    "An untracked server is listening on port {}.",
                    state.web.port,
                ),
                Style::default().fg(state.theme.priority_p1),
            ));
            lines.push(Line::styled(
                "Stop it in the terminal that launched it.",
                Style::default().fg(muted),
            ));
        }
        _ => lines.push(Line::styled(
            format!("Start port: {}", state.web.port),
            Style::default().fg(muted),
        )),
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Logs: ", Style::default().fg(muted)),
        Span::styled(
            "totui web logs --follow | lux",
            Style::default().fg(state.theme.description_indicator),
        ),
    ]));
    lines.push(Line::styled(
        "The server keeps running when you close the TUI.",
        Style::default().fg(muted),
    ));
    if let Some(error) = &state.web.error {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            error.clone(),
            Style::default().fg(state.theme.priority_p0),
        ));
    }
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .style(
                Style::default()
                    .fg(state.theme.foreground)
                    .bg(state.theme.background),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(accent))
                    .title(Line::styled(
                        " Web server ",
                        Style::default().fg(accent).add_modifier(Modifier::BOLD),
                    )),
            ),
        area,
    );
}
