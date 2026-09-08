use super::{AppState, Mode};
use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent};
use serde_json::Value;
use to_tui::remote::{cache::Conflict, protocol::TaskResource};

#[derive(Debug, Clone)]
pub struct Field {
    pub name: &'static str,
    path: &'static str,
    pub mine: String,
    pub server: String,
    pub choice: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct SyncDialog {
    pub conflict: Conflict,
    pub fields: Vec<Field>,
    pub selected: usize,
    pub scroll: u16,
    pub combining: bool,
    pub message: String,
}

impl SyncDialog {
    pub fn new(conflict: Conflict) -> Self {
        let mine = serde_json::to_value(&conflict.local).unwrap_or(Value::Null);
        let server =
            serde_json::to_value(conflict.server.as_ref().and_then(|s| s.resource.as_ref()))
                .unwrap_or(Value::Null);
        let fields = [
            ("Content", "/item/content"),
            ("Description", "/item/description"),
            ("State", "/item/state"),
            ("Priority", "/item/priority"),
            ("Due date", "/item/due_date"),
            ("Parent", "/item/parent_id"),
            ("Indent", "/item/indent_level"),
            ("Collapsed", "/item/collapsed"),
            ("Project", "/project"),
            ("Date", "/date"),
            ("Position", "/position"),
        ]
        .into_iter()
        .filter(|(_, path)| mine.pointer(path) != server.pointer(path))
        .map(|(name, path)| Field {
            name,
            path,
            mine: display(mine.pointer(path)),
            server: display(server.pointer(path)),
            choice: None,
        })
        .collect();
        Self {
            conflict,
            fields,
            selected: 0,
            scroll: 0,
            combining: false,
            message: String::new(),
        }
    }

    fn combined(&self) -> Result<Option<TaskResource>> {
        ensure!(
            self.fields.iter().all(|f| f.choice.is_some()),
            "Choose mine or server for every field first"
        );
        let mine = serde_json::to_value(&self.conflict.local)?;
        let mut server = serde_json::to_value(
            self.conflict
                .server
                .as_ref()
                .and_then(|s| s.resource.as_ref()),
        )?;
        ensure!(
            !mine.is_null() && !server.is_null(),
            "Choose a whole version when a task was deleted"
        );
        for field in &self.fields {
            if field.choice == Some(true) {
                *server
                    .pointer_mut(field.path)
                    .context("Missing task field")? = mine
                    .pointer(field.path)
                    .context("Missing local field")?
                    .clone();
            }
        }
        let mut resource: TaskResource = serde_json::from_value(server)?;
        resource.item.modified_at = chrono::Utc::now();
        resource.item.completed_at = if resource.item.state.is_complete() {
            resource
                .item
                .completed_at
                .or(Some(resource.item.modified_at))
        } else {
            None
        };
        Ok(Some(resource))
    }
}

fn display(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) => "(none)".into(),
        Some(value) => value.to_string(),
        None => "(deleted)".into(),
    }
}

pub fn poll(state: &mut AppState, force: bool) {
    if state.sync_dialog.is_some()
        || state.mode != Mode::Navigate
        || state.unsaved_changes
        || state.show_help
    {
        return;
    }
    if let Some(cache) = to_tui::remote::active().and_then(|c| c.cached()) {
        if let Some(conflict) = cache.conflict() {
            if force || state.sync_dismissed != Some(conflict.id) {
                state.sync_dialog = Some(SyncDialog::new(conflict));
            }
        } else if force {
            let status = cache.status();
            state.set_status_message(
                status
                    .error
                    .unwrap_or_else(|| format!("{} edits queued for sync", status.pending)),
            );
        }
    }
}

pub fn handle(key: KeyEvent, state: &mut AppState) -> Result<()> {
    let Some(dialog) = state.sync_dialog.as_mut() else {
        return Ok(());
    };
    let mut resolution = None;
    let previous = dialog.selected;
    match key.code {
        KeyCode::PageDown => dialog.scroll = dialog.scroll.saturating_add(5),
        KeyCode::PageUp => dialog.scroll = dialog.scroll.saturating_sub(5),
        KeyCode::Esc => {
            state.sync_dismissed = Some(dialog.conflict.id);
            state.sync_dialog = None;
            return Ok(());
        }
        KeyCode::Down | KeyCode::Char('j') => {
            dialog.selected = (dialog.selected + 1).min(dialog.fields.len().saturating_sub(1))
        }
        KeyCode::Up | KeyCode::Char('k') => dialog.selected = dialog.selected.saturating_sub(1),
        KeyCode::Char('m') if !dialog.combining => {
            if dialog.conflict.local.is_some()
                && dialog
                    .conflict
                    .server
                    .as_ref()
                    .is_some_and(|s| s.resource.is_some())
            {
                dialog.combining = true;
            } else {
                dialog.message = "One version is deleted; choose mine or server.".into();
            }
        }
        KeyCode::Char(c @ ('l' | 's')) => {
            if dialog.combining {
                if let Some(field) = dialog.fields.get_mut(dialog.selected) {
                    field.choice = Some(c == 'l');
                }
            } else {
                resolution = Some(if c == 'l' {
                    dialog.conflict.local.clone()
                } else {
                    dialog
                        .conflict
                        .server
                        .as_ref()
                        .and_then(|s| s.resource.clone())
                });
            }
        }
        KeyCode::Enter if dialog.combining => match dialog.combined() {
            Ok(resource) => resolution = Some(resource),
            Err(error) => dialog.message = error.to_string(),
        },
        _ => {}
    }
    if dialog.selected != previous {
        dialog.scroll = 0;
    }
    if let Some(resource) = resolution {
        let cache = to_tui::remote::active()
            .and_then(|c| c.cached())
            .context("Remote cache unavailable")?;
        match cache.resolve(&dialog.conflict, resource) {
            Ok(()) => {
                state.sync_dialog = None;
                state.sync_dismissed = None;
                state.reload_from_database()?;
                state.set_status_message("Resolution saved locally; syncing in background".into());
            }
            Err(error) => {
                let message = error.to_string();
                state.sync_dialog = cache.conflict().map(SyncDialog::new);
                if let Some(dialog) = state.sync_dialog.as_mut() {
                    dialog.message = message;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use to_tui::remote::protocol::TaskVersion;
    use to_tui::todo::TodoItem;

    #[test]
    fn test_manual_combination_requires_explicit_choices() {
        let base = TaskResource {
            project: "default".into(),
            date: chrono::Local::now().date_naive(),
            position: 0,
            item: TodoItem::new("base".into(), 0),
        };
        let mut local = base.clone();
        local.item.content = "mine".into();
        let mut server = base.clone();
        server.item.description = Some("server notes".into());
        let mut dialog = SyncDialog::new(Conflict {
            id: base.item.id,
            base: Some(base),
            local: Some(local),
            server: Some(TaskVersion {
                id: server.item.id,
                etag: "v2".into(),
                resource: Some(server),
            }),
        });
        assert!(dialog.combined().is_err());
        for field in &mut dialog.fields {
            field.choice = Some(field.name == "Content");
        }
        let result = dialog.combined().unwrap().unwrap();
        assert_eq!(
            (
                result.item.content.as_str(),
                result.item.description.as_deref()
            ),
            ("mine", Some("server notes"))
        );
    }
}
