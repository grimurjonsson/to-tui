use crate::utils::unicode::{next_char_boundary, prev_char_boundary};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use std::time::{Duration, Instant};
use to_tui::kanban::{Action, Board, Request, Status, Ticket};

const COLUMNS: [(Status, &str); 6] = [
    (Status::Backlog, "Backlog"),
    (Status::Ready, "Ready"),
    (Status::InProgress, "In progress"),
    (Status::Review, "Review"),
    (Status::Done, "Done"),
    (Status::Blocked, "Blocked"),
];

#[derive(Debug, Clone)]
enum FormKind {
    Board,
    New,
    Edit(Ticket),
    Move(Ticket, Status),
    Comment(Ticket),
    Resolve(Ticket),
}

#[derive(Debug, Clone)]
struct Form {
    kind: FormKind,
    fields: Vec<(&'static str, String)>,
    selected: usize,
    cursor: usize,
}

#[derive(Debug, Clone)]
pub struct KanbanUi {
    project: String,
    board: Option<Board>,
    column: usize,
    row: usize,
    detail_scroll: u16,
    form: Option<Form>,
    error: Option<String>,
    refreshed: Instant,
}

impl KanbanUi {
    pub fn new(project: String) -> Self {
        let mut ui = Self {
            project,
            board: None,
            column: 1,
            row: 0,
            detail_scroll: 0,
            form: None,
            error: None,
            refreshed: Instant::now(),
        };
        ui.refresh();
        ui
    }

    pub fn tick(&mut self) {
        if self.refreshed.elapsed() >= Duration::from_secs(1) {
            self.refresh();
        }
    }

    fn request(&self, action: Action) -> anyhow::Result<Option<Board>> {
        to_tui::kanban::execute(Request {
            project: self.project.clone(),
            actor: "user (TUI)".into(),
            action,
        })
    }

    fn refresh(&mut self) {
        self.refreshed = Instant::now();
        let selected = self.ticket().map(|ticket| ticket.id.clone());
        match self.request(Action::View) {
            Ok(board) => {
                self.board = board;
                if let Some(id) = selected {
                    self.select_id(&id);
                }
                self.clamp_row();
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn tickets_in(&self, column: usize) -> Vec<&Ticket> {
        self.board
            .as_ref()
            .map(|b| {
                b.tickets
                    .iter()
                    .filter(|t| {
                        if column == 7 {
                            t.archived
                        } else if column == 6 {
                            t.trashed
                        } else {
                            !t.trashed && !t.archived && t.status == COLUMNS[column].0
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn tickets(&self) -> Vec<&Ticket> {
        self.tickets_in(self.column)
    }

    fn ticket(&self) -> Option<&Ticket> {
        self.tickets().get(self.row).copied()
    }

    fn select_id(&mut self, id: &str) {
        if let Some(ticket) = self
            .board
            .as_ref()
            .and_then(|b| b.tickets.iter().find(|t| t.id == id))
        {
            self.column = if ticket.archived {
                7
            } else if ticket.trashed {
                6
            } else {
                COLUMNS
                    .iter()
                    .position(|(status, _)| *status == ticket.status)
                    .unwrap_or(0)
            };
            self.row = self.tickets().iter().position(|t| t.id == id).unwrap_or(0);
        }
    }

    fn clamp_row(&mut self) {
        self.row = self.row.min(self.tickets().len().saturating_sub(1));
    }

    fn open_form(&mut self, kind: FormKind) {
        let fields = match &kind {
            FormKind::Board => vec![("Board name", "Delivery".into())],
            FormKind::New => vec![
                ("Title", String::new()),
                ("Description", String::new()),
                ("Assignee", String::new()),
            ],
            FormKind::Edit(ticket) => vec![
                ("Title", ticket.title.clone()),
                ("Description", ticket.description.clone()),
                ("Assignee", ticket.assignee.clone().unwrap_or_default()),
            ],
            FormKind::Move(_, _) => {
                vec![("Move reason (required when moving back)", String::new())]
            }
            FormKind::Comment(_) => vec![("Comment", String::new())],
            FormKind::Resolve(_) => vec![("How was the feedback addressed?", String::new())],
        };
        self.error = None;
        let cursor = fields[0].1.len();
        self.form = Some(Form {
            cursor,
            kind,
            fields,
            selected: 0,
        });
    }

    fn submit(&mut self) {
        let Some(form) = &self.form else {
            return;
        };
        let value = |index: usize| form.fields[index].1.clone();
        let optional = |index| {
            let text = value(index);
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        };
        let mut selected = self.ticket().map(|t| t.id.clone());
        let action = match &form.kind {
            FormKind::Board => Action::CreateBoard { name: value(0) },
            FormKind::New => Action::CreateTicket {
                title: value(0),
                description: value(1),
                assignee: optional(2),
            },
            FormKind::Edit(t) => Action::EditTicket {
                id: t.id.clone(),
                expected_revision: t.revision,
                title: value(0),
                description: value(1),
                assignee: optional(2),
            },
            FormKind::Move(t, status) => Action::MoveTicket {
                id: t.id.clone(),
                expected_revision: t.revision,
                status: *status,
                reason: optional(0),
            },
            FormKind::Comment(t) => Action::Comment {
                id: t.id.clone(),
                expected_revision: t.revision,
                body: value(0),
            },
            FormKind::Resolve(t) => Action::AddressFeedback {
                id: t.id.clone(),
                expected_revision: t.revision,
                resolution: value(0),
            },
        };
        match self.request(action) {
            Ok(board) => {
                if matches!(form.kind, FormKind::New) {
                    selected = board
                        .as_ref()
                        .and_then(|b| b.tickets.last())
                        .map(|t| t.id.clone());
                }
                self.board = board;
                self.form = None;
                self.error = None;
                self.detail_scroll = 0;
                if let Some(id) = selected {
                    self.select_id(&id);
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> bool {
        if self.form.is_some() {
            if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                self.submit();
                return false;
            }
            if let Some(form) = &mut self.form {
                match key.code {
                    KeyCode::Esc => {
                        self.form = None;
                        self.error = None;
                    }
                    KeyCode::Tab => {
                        form.selected = (form.selected + 1) % form.fields.len();
                        form.cursor = form.fields[form.selected].1.len();
                    }
                    KeyCode::BackTab => {
                        form.selected = (form.selected + form.fields.len() - 1) % form.fields.len();
                        form.cursor = form.fields[form.selected].1.len();
                    }
                    KeyCode::Backspace => {
                        let previous =
                            prev_char_boundary(&form.fields[form.selected].1, form.cursor);
                        form.fields[form.selected]
                            .1
                            .replace_range(previous..form.cursor, "");
                        form.cursor = previous;
                    }
                    KeyCode::Enter => {
                        form.fields[form.selected].1.insert(form.cursor, '\n');
                        form.cursor += 1;
                    }
                    KeyCode::Left => {
                        form.cursor = prev_char_boundary(&form.fields[form.selected].1, form.cursor)
                    }
                    KeyCode::Right => {
                        form.cursor = next_char_boundary(&form.fields[form.selected].1, form.cursor)
                    }
                    KeyCode::Home => form.cursor = 0,
                    KeyCode::End => form.cursor = form.fields[form.selected].1.len(),
                    KeyCode::Delete => {
                        let next = next_char_boundary(&form.fields[form.selected].1, form.cursor);
                        form.fields[form.selected]
                            .1
                            .replace_range(form.cursor..next, "");
                    }
                    KeyCode::Char(c)
                        if !key.modifiers.intersects(
                            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                        ) =>
                    {
                        form.fields[form.selected].1.insert(form.cursor, c);
                        form.cursor += c.len_utf8();
                    }
                    _ => {}
                }
            }
            return false;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(7) => return true,
            KeyCode::Left | KeyCode::Char('h') => {
                self.column = self.column.saturating_sub(1);
                self.row = 0;
                self.detail_scroll = 0;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.column = (self.column + 1).min(5);
                self.row = 0;
                self.detail_scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.row = self.row.saturating_sub(1);
                self.detail_scroll = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.row += 1;
                self.clamp_row();
                self.detail_scroll = 0;
            }
            KeyCode::Tab => {
                self.column = if self.column == 0 { 1 } else { 0 };
                self.row = 0;
                self.detail_scroll = 0;
            }
            KeyCode::Char('v') => {
                self.column = if self.column == 7 { 1 } else { 7 };
                self.row = 0;
                self.detail_scroll = 0;
            }
            KeyCode::Char('t') => {
                self.column = if self.column == 6 { 1 } else { 6 };
                self.row = 0;
                self.detail_scroll = 0;
            }
            KeyCode::PageDown => self.detail_scroll = self.detail_scroll.saturating_add(5),
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(5),
            KeyCode::Char('r') => {
                self.error = None;
                self.refresh();
            }
            KeyCode::Char('c') if self.board.is_none() => self.open_form(FormKind::Board),
            KeyCode::Char('n') if self.board.is_some() => self.open_form(FormKind::New),
            code => {
                if let Some(ticket) = self.ticket().cloned() {
                    if code == KeyCode::Char('z')
                        && ticket.status == Status::Done
                        && !ticket.trashed
                    {
                        let action = if ticket.archived {
                            Action::UnarchiveTicket {
                                id: ticket.id.clone(),
                                expected_revision: ticket.revision,
                            }
                        } else {
                            Action::ArchiveTicket {
                                id: ticket.id.clone(),
                                expected_revision: ticket.revision,
                            }
                        };
                        match self.request(action) {
                            Ok(board) => {
                                self.board = board;
                                self.error = None;
                                self.clamp_row();
                            }
                            Err(error) => self.error = Some(error.to_string()),
                        }
                        return false;
                    }
                    if ticket.archived {
                        return false;
                    }
                    if (!ticket.trashed
                        && ticket.status == Status::Backlog
                        && code == KeyCode::Char('x'))
                        || (ticket.trashed && code == KeyCode::Char('u'))
                    {
                        let action = if ticket.trashed {
                            Action::RestoreTicket {
                                id: ticket.id.clone(),
                                expected_revision: ticket.revision,
                            }
                        } else {
                            Action::TrashTicket {
                                id: ticket.id.clone(),
                                expected_revision: ticket.revision,
                            }
                        };
                        match self.request(action) {
                            Ok(board) => {
                                self.board = board;
                                self.error = None;
                                self.clamp_row();
                            }
                            Err(error) => self.error = Some(error.to_string()),
                        }
                        return false;
                    }
                    if ticket.trashed {
                        return false;
                    }
                    match code {
                        KeyCode::Char('b') => {
                            self.open_form(FormKind::Move(ticket, Status::Backlog))
                        }
                        KeyCode::Char('e') => self.open_form(FormKind::Edit(ticket)),
                        KeyCode::Char('c') => self.open_form(FormKind::Comment(ticket)),
                        KeyCode::Char('a') => self.open_form(FormKind::Resolve(ticket)),
                        KeyCode::Char(number @ '1'..='6') => self.open_form(FormKind::Move(
                            ticket,
                            COLUMNS[number as usize - '1' as usize].0,
                        )),
                        _ => {}
                    }
                }
            }
        }
        false
    }

    pub fn render(&self, frame: &mut Frame) {
        let areas = Layout::vertical([
            Constraint::Length(2),
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(frame.area());
        frame.render_widget(
            Paragraph::new(format!(
                "{} · {}",
                self.project,
                self.board
                    .as_ref()
                    .map(|b| b.name.as_str())
                    .unwrap_or("No kanban board")
            ))
            .style(Style::default().fg(Color::Cyan)),
            areas[0],
        );
        let per_row = (areas[1].width as usize / 20).clamp(1, 5);
        let row_count = 5_usize.div_ceil(per_row);
        let rows = Layout::vertical(vec![Constraint::Ratio(1, row_count as u32); row_count])
            .split(areas[1]);
        for (row, area) in rows.iter().enumerate() {
            let count = (5 - row * per_row).min(per_row);
            let columns =
                Layout::horizontal(vec![Constraint::Ratio(1, count as u32); count]).split(*area);
            for (offset, column) in columns.iter().enumerate() {
                self.render_column(frame, *column, 1 + row * per_row + offset);
            }
        }
        self.render_column(
            frame,
            areas[2],
            if self.column >= 6 { self.column } else { 0 },
        );
        let details = if let Some(t) = self.ticket() {
            let mut text = format!(
                "{}\n{} · revision {}\n{}\n\n{}\n\n",
                t.title,
                t.id,
                t.revision,
                t.description,
                t.feedback
                    .as_deref()
                    .map(|f| format!("Outstanding feedback: {f}"))
                    .unwrap_or_else(|| "No outstanding move feedback".into())
            );
            for event in t.activity.iter().rev() {
                text.push_str(&format!(
                    "{} · {} · {}\n{}\n\n",
                    event.at, event.actor, event.action, event.body
                ));
            }
            text
        } else if self.board.is_none() {
            "Press c to create a board for this project.".into()
        } else {
            "No ticket selected. Press n to create one.".into()
        };
        frame.render_widget(
            Paragraph::new(details)
                .wrap(Wrap { trim: false })
                .scroll((self.detail_scroll, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Ticket and activity · PgUp/PgDn scroll"),
                ),
            areas[3],
        );
        frame.render_widget(Paragraph::new(self.error.clone().unwrap_or_else(|| "←/→ columns  ↑/↓ tickets  n new  e edit  c comment  a resolve feedback\nb To backlog · x Trash · t Trash · u restore · Tab backlog\n1–6 move · z archive/restore Done · v Completed · Esc return".into())).wrap(Wrap { trim: false }), areas[4]);
        if let Some(form) = &self.form {
            self.render_form(frame, form);
        }
    }

    fn render_column(&self, frame: &mut Frame, area: Rect, index: usize) {
        let items: Vec<ListItem> = self
            .tickets_in(index)
            .iter()
            .map(|t| {
                ListItem::new(format!(
                    "{}{}\n  {}",
                    if t.feedback.is_some() { "! " } else { "" },
                    t.title.replace('\n', " "),
                    t.assignee.as_deref().unwrap_or("Unassigned")
                ))
            })
            .collect();
        let name = if index == 7 {
            "Completed · z restore"
        } else if index == 6 {
            "Trash · u restore"
        } else {
            COLUMNS[index].1
        };
        let title = if index >= 6 {
            format!("{} ({})", name, items.len())
        } else {
            format!("{} {} ({})", index + 1, name, items.len())
        };
        let color = if index == self.column {
            Color::Cyan
        } else {
            Color::DarkGray
        };
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(color)),
            )
            .highlight_style(Style::default().bg(Color::DarkGray))
            .highlight_symbol("› ");
        let mut selected = ListState::default();
        if index == self.column && self.ticket().is_some() {
            selected.select(Some(self.row));
        }
        frame.render_stateful_widget(list, area, &mut selected);
    }

    fn render_form(&self, frame: &mut Frame, form: &Form) {
        let area = frame.area();
        let width = area.width.saturating_sub(4).min(90);
        let height = area.height.saturating_sub(2).min(24);
        let popup = Rect::new(
            (area.width - width) / 2,
            (area.height - height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, popup);
        let title = match &form.kind {
            FormKind::Board => "Create board".into(),
            FormKind::New => "New ticket".into(),
            FormKind::Edit(_) => "Edit ticket".into(),
            FormKind::Move(_, status) => format!(
                "Move to {}",
                COLUMNS
                    .iter()
                    .find(|(s, _)| s == status)
                    .map(|(_, name)| *name)
                    .unwrap_or("column")
            ),
            FormKind::Comment(_) => "Add comment".into(),
            FormKind::Resolve(_) => "Address feedback".into(),
        };
        let block = Block::default().borders(Borders::ALL).title(title);
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        let layout = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(2),
            Constraint::Length(4),
        ])
        .split(inner);
        let labels = form
            .fields
            .iter()
            .enumerate()
            .map(|(index, (name, _))| {
                format!("{} {name}", if index == form.selected { "›" } else { " " })
            })
            .collect::<Vec<_>>()
            .join("  ");
        frame.render_widget(
            Paragraph::new(labels)
                .style(Style::default().fg(Color::Cyan))
                .wrap(Wrap { trim: false }),
            layout[0],
        );
        let value = &form.fields[form.selected].1;
        let display = format!("{}▏{}", &value[..form.cursor], &value[form.cursor..]);
        let caret_line = Paragraph::new(format!("{}▏", &value[..form.cursor]))
            .wrap(Wrap { trim: false })
            .line_count(layout[1].width.max(1));
        let scroll = caret_line
            .saturating_sub(layout[1].height as usize)
            .min(u16::MAX as usize) as u16;
        frame.render_widget(
            Paragraph::new(display)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0)),
            layout[1],
        );
        frame.render_widget(
            Paragraph::new(format!(
                "Tab next field · Enter newline · Ctrl+S save · Esc cancel\n{}",
                self.error.as_deref().unwrap_or("")
            ))
            .wrap(Wrap { trim: false }),
            layout[2],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ratatui::{Terminal, backend::TestBackend};
    use std::process::Command;

    fn key(ui: &mut KanbanUi, code: KeyCode) {
        ui.handle(KeyEvent::new(code, KeyModifiers::NONE));
    }
    fn type_text(ui: &mut KanbanUi, value: &str) {
        for c in value.chars() {
            key(ui, KeyCode::Char(c));
        }
    }
    fn save(ui: &mut KanbanUi) {
        ui.handle(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    }

    #[test]
    fn test_kanban_tui_isolated_workflow() {
        let dir = tempfile::tempdir().unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "app::kanban::tests::test_kanban_tui_worker",
                "--nocapture",
            ])
            .env("TOTUI_DATA_DIR", dir.path())
            .env("TOTUI_KANBAN_TUI_TEST", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn test_kanban_tui_worker() {
        if std::env::var("TOTUI_KANBAN_TUI_TEST").is_err() {
            return;
        }
        to_tui::storage::database::init_database().unwrap();
        to_tui::storage::database::ensure_default_project_exists().unwrap();
        let mut ui = KanbanUi::new("default".into());
        key(&mut ui, KeyCode::Char('c'));
        save(&mut ui);
        assert_eq!(ui.board.as_ref().unwrap().name, "Delivery");
        key(&mut ui, KeyCode::Char('n'));
        type_text(&mut ui, "Test keyboard workflow");
        key(&mut ui, KeyCode::Tab);
        type_text(&mut ui, "Keep this description");
        save(&mut ui);
        let id = ui.ticket().unwrap().id.clone();
        key(&mut ui, KeyCode::Char('5'));
        save(&mut ui);
        assert_eq!(ui.ticket().unwrap().status, Status::Done);
        key(&mut ui, KeyCode::Char('2'));
        save(&mut ui);
        assert!(ui.error.as_ref().unwrap().contains("Reason"));
        type_text(&mut ui, "Cover Unicode input");
        save(&mut ui);
        assert_eq!(
            ui.ticket().unwrap().feedback.as_deref(),
            Some("Cover Unicode input")
        );
        key(&mut ui, KeyCode::Char('c'));
        type_text(&mut ui, "Draft comment");
        ui.request(Action::Comment {
            id: id.clone(),
            expected_revision: 3,
            body: "Agent changed ticket".into(),
        })
        .unwrap();
        ui.refresh();
        save(&mut ui);
        assert!(ui.error.as_ref().unwrap().contains("Ticket changed"));
        assert_eq!(ui.form.as_ref().unwrap().fields[0].1, "Draft comment");
        key(&mut ui, KeyCode::Esc);
        key(&mut ui, KeyCode::Char('c'));
        type_text(&mut ui, "User comment after reading agent update");
        save(&mut ui);
        key(&mut ui, KeyCode::Char('a'));
        type_text(&mut ui, "Verified Unicode editing");
        save(&mut ui);
        key(&mut ui, KeyCode::Char('5'));
        save(&mut ui);
        assert_eq!(ui.ticket().unwrap().status, Status::Done);
        let board = ui.request(Action::View).unwrap().unwrap();
        assert!(
            board.tickets[0]
                .activity
                .iter()
                .any(|event| event.body == "Cover Unicode input")
        );
        assert!(board.tickets[0].feedback.is_none());
        key(&mut ui, KeyCode::Char('z'));
        assert!(ui.ticket().is_none());
        key(&mut ui, KeyCode::Char('v'));
        assert!(ui.ticket().unwrap().archived);
        key(&mut ui, KeyCode::Char('z'));
        ui.select_id(&id);
        assert!(!ui.ticket().unwrap().archived);
        key(&mut ui, KeyCode::Char('b'));
        type_text(&mut ui, "Defer to next week");
        save(&mut ui);
        assert_eq!(ui.ticket().unwrap().status, Status::Backlog);
        key(&mut ui, KeyCode::Char('x'));
        assert!(ui.ticket().is_none());
        key(&mut ui, KeyCode::Char('t'));
        assert!(ui.ticket().unwrap().trashed);
        key(&mut ui, KeyCode::Char('u'));
        key(&mut ui, KeyCode::Tab);
        assert!(!ui.ticket().unwrap().trashed);
        assert_eq!(
            ui.ticket().unwrap().feedback.as_deref(),
            Some("Defer to next week")
        );
        for (width, height) in [(20, 8), (80, 24), (160, 40)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| ui.render(frame)).unwrap();
            key(&mut ui, KeyCode::Char('e'));
            terminal.draw(|frame| ui.render(frame)).unwrap();
            key(&mut ui, KeyCode::Esc);
        }
    }

    #[test]
    fn test_unicode_form_edits_keep_valid_boundaries() {
        let mut ui = KanbanUi {
            project: "test".into(),
            board: None,
            column: 0,
            row: 0,
            detail_scroll: 0,
            form: None,
            error: None,
            refreshed: Instant::now(),
        };
        ui.open_form(FormKind::New);
        type_text(&mut ui, "a🙂é");
        key(&mut ui, KeyCode::Left);
        key(&mut ui, KeyCode::Backspace);
        assert_eq!(ui.form.as_ref().unwrap().fields[0].1, "aé");
        type_text(&mut ui, "界");
        key(&mut ui, KeyCode::Delete);
        assert_eq!(ui.form.as_ref().unwrap().fields[0].1, "a界");
        key(&mut ui, KeyCode::Home);
        key(&mut ui, KeyCode::Backspace);
        assert_eq!(ui.form.as_ref().unwrap().cursor, 0);
        key(&mut ui, KeyCode::Tab);
        type_text(&mut ui, "Description");
        key(&mut ui, KeyCode::BackTab);
        assert_eq!(ui.form.as_ref().unwrap().cursor, "a界".len());
    }
}
