use super::protocol::{Request, Snapshot};
use crate::storage::{context, database, file, rollover};
use crate::utils::paths::get_daily_file_path_for_project;
use anyhow::{Result, ensure};
use axum::{
    Json,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Local;
use serde_json::{Value, json};
use std::collections::HashSet;

fn project_name(name: &str) -> Result<()> {
    ensure!(
        !name.trim().is_empty()
            && name != "."
            && name != ".."
            && name.len() <= 100
            && !name
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\')),
        "Invalid project name"
    );
    Ok(())
}

fn existing_project(name: &str) -> Result<()> {
    project_name(name)?;
    ensure!(
        database::get_project_by_name(name)?.is_some(),
        "Project does not exist"
    );
    Ok(())
}

fn change_project(name: &str, new_name: Option<&str>) -> Result<()> {
    existing_project(name)?;
    ensure!(
        name != "default",
        "Cannot rename or delete the default project"
    );
    if let Some(new) = new_name {
        project_name(new)?;
        if new == name {
            return Ok(());
        }
        ensure!(
            database::get_project_by_name(new)?.is_none(),
            "Project already exists"
        );
    }
    let old_path = crate::utils::paths::get_project_dir(name)?;
    let destination = crate::utils::paths::get_project_dir(
        new_name.unwrap_or(&format!(".deleted-{}", uuid::Uuid::new_v4())),
    )?;
    ensure!(
        !destination.exists(),
        "Destination project directory already exists"
    );
    let moved = old_path.exists();
    if moved {
        std::fs::rename(&old_path, &destination)?;
    }
    let result = match new_name {
        Some(new) => database::rename_project(name, new),
        None => database::delete_project(name),
    };
    if let Err(error) = result {
        if moved {
            std::fs::rename(&destination, &old_path)?;
        }
        return Err(error);
    }
    if moved && new_name.is_none() {
        std::fs::remove_dir_all(destination)?;
    }
    Ok(())
}

fn execute(request: Request) -> Result<Value> {
    match request {
        Request::Info => {
            Ok(json!({"protocol":1, "sync":1, "stream":1, "today":Local::now().date_naive()}))
        }
        Request::Load {
            project,
            date,
            history,
        } => {
            existing_project(&project)?;
            let list = if history {
                file::load_todos_for_viewing_in_project(&project, date)?
            } else {
                file::load_todo_list_for_project(&project, date)?
            };
            Ok(serde_json::to_value(Snapshot::from(&list))?)
        }
        Request::Exists { project, date } => {
            existing_project(&project)?;
            Ok(json!(file::file_exists_for_project(&project, date)?))
        }
        Request::Save { lists } => {
            ensure!(
                !lists.is_empty() && lists.len() <= 2,
                "Save requires one or two lists"
            );
            let mut keys = HashSet::new();
            let mut ids = HashSet::new();
            for (project, snapshot) in &lists {
                existing_project(project)?;
                ensure!(
                    snapshot.date == Local::now().date_naive(),
                    "Conflict: Today changed; reload before saving"
                );
                ensure!(keys.insert((project, snapshot.date)), "Duplicate list");
                let mut parents = HashSet::new();
                for item in &snapshot.items {
                    ensure!(ids.insert(item.id), "Duplicate task ID");
                    ensure!(
                        item.parent_id.is_none_or(|id| parents.contains(&id)),
                        "Parent must precede its child"
                    );
                    parents.insert(item.id);
                }
            }
            let lists = lists
                .into_iter()
                .map(|(project, snapshot)| {
                    let path = get_daily_file_path_for_project(&project, snapshot.date)?;
                    Ok((project, snapshot.into_list(path)))
                })
                .collect::<Result<Vec<_>>>()?;
            let refs = lists
                .iter()
                .map(|(project, list)| (list, project.as_str()))
                .collect::<Vec<_>>();
            database::save_lists_atomically(&refs)?;
            for (project, list) in &lists {
                file::export_committed_list(list, project);
            }
            Ok(json!(
                lists
                    .iter()
                    .map(|(_, list)| list.revision.get())
                    .collect::<Vec<_>>()
            ))
        }
        Request::Candidates { project } => {
            existing_project(&project)?;
            Ok(serde_json::to_value(
                rollover::find_rollover_candidates_for_project(&project)?,
            )?)
        }
        Request::Rollover {
            project,
            source_date,
            items,
        } => {
            existing_project(&project)?;
            ensure!(
                source_date < Local::now().date_naive(),
                "Rollover requires a prior date"
            );
            let list = rollover::execute_rollover_for_project(&project, source_date, items)?;
            Ok(serde_json::to_value(Snapshot::from(&list))?)
        }
        Request::Projects => Ok(serde_json::to_value(database::load_projects()?)?),
        Request::CreateProject { project } => {
            project_name(&project.name)?;
            database::create_project(&project)?;
            Ok(Value::Null)
        }
        Request::RenameProject { old, new } => {
            change_project(&old, Some(&new))?;
            Ok(Value::Null)
        }
        Request::DeleteProject { name } => {
            change_project(&name, None)?;
            Ok(Value::Null)
        }
    }
}

pub async fn handle(headers: HeaderMap, Json(request): Json<Request>) -> Response {
    if !matches!(request, Request::Info) && !headers.contains_key("x-totui-expected-user") {
        return (
            StatusCode::BAD_REQUEST,
            "Remote requests require an expected account",
        )
            .into_response();
    }
    match context::blocking(move || execute(request)).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => {
            let message = error.to_string();
            let status = if message.starts_with("Conflict:") {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            (status, Json(json!({"error":message}))).into_response()
        }
        Err(error) => super::super::api::models::ErrorResponse::internal(error),
    }
}
