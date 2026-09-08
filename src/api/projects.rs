use super::models::{ErrorResponse, ProjectResponse};
use crate::config::Config;
use crate::project::{DEFAULT_PROJECT_NAME, ProjectRegistry};
use crate::utils::paths::get_project_dir;
use anyhow::Result;
use axum::{
    Json,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectName {
    pub name: String,
}

fn rejection(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(ErrorResponse::new(message))).into_response()
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 100
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\'))
}

pub async fn create(Json(input): Json<ProjectName>) -> Response {
    let name = input.name.trim();
    if !valid_name(name) {
        return rejection(
            StatusCode::BAD_REQUEST,
            "Enter a short project name without slashes or control characters.",
        );
    }
    let mut registry = match ProjectRegistry::load() {
        Ok(registry) => registry,
        Err(error) => return ErrorResponse::internal(error),
    };
    if registry.get_by_name(name).is_some() {
        return rejection(
            StatusCode::CONFLICT,
            "A project with that name already exists.",
        );
    }
    match registry.create(name) {
        Ok(project) => (StatusCode::CREATED, Json(ProjectResponse::from(project))).into_response(),
        Err(error) => ErrorResponse::internal(error),
    }
}

pub async fn rename(Path(id): Path<Uuid>, Json(input): Json<ProjectName>) -> Response {
    change(id, Some(input.name.trim().to_owned()))
}

pub async fn delete(Path(id): Path<Uuid>) -> Response {
    change(id, None)
}

fn change(id: Uuid, new_name: Option<String>) -> Response {
    if new_name.as_deref().is_some_and(|name| !valid_name(name)) {
        return rejection(
            StatusCode::BAD_REQUEST,
            "Enter a short project name without slashes or control characters.",
        );
    }
    let mut registry = match ProjectRegistry::load() {
        Ok(registry) => registry,
        Err(error) => return ErrorResponse::internal(error),
    };
    let Some(project) = registry
        .projects
        .iter()
        .find(|project| project.id == id)
        .cloned()
    else {
        return rejection(StatusCode::NOT_FOUND, "This project no longer exists.");
    };
    if project.name == DEFAULT_PROJECT_NAME {
        return rejection(
            StatusCode::BAD_REQUEST,
            "The default project cannot be renamed or deleted.",
        );
    }
    if !valid_name(&project.name) {
        return rejection(
            StatusCode::BAD_REQUEST,
            "This project's name must be corrected before managing its files.",
        );
    }
    if let Some(name) = &new_name {
        if name == &project.name {
            return Json(ProjectResponse::from(&project)).into_response();
        }
        if registry.get_by_name(name).is_some() {
            return rejection(
                StatusCode::CONFLICT,
                "A project with that name already exists.",
            );
        }
    }
    let result = (|| -> Result<()> {
        let old_dir = get_project_dir(&project.name)?;
        let destination =
            get_project_dir(new_name.as_deref().unwrap_or(&format!(".deleted-{id}")))?;
        anyhow::ensure!(
            !destination.exists(),
            "Destination project directory already exists"
        );
        let moved = old_dir.exists();
        if moved {
            std::fs::rename(&old_dir, &destination)?;
        }
        let result = match &new_name {
            Some(name) => registry.rename(&project.name, name),
            None => registry.delete(&project.name),
        };
        if let Err(error) = result {
            if moved {
                std::fs::rename(&destination, &old_dir)?;
            }
            return Err(error);
        }
        if new_name.is_none() && moved {
            std::fs::remove_dir_all(destination)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        return ErrorResponse::internal(error);
    }
    let update_config = (|| -> Result<()> {
        let mut config = Config::load()?;
        match &new_name {
            Some(name) => config.rebind_project_name(&project.name, name),
            None => config.unbind_project(&project.name),
        }
        if config.last_used_project.as_deref() == Some(&project.name) {
            config.last_used_project = new_name.clone();
        }
        config.save()
    })();
    if let Err(error) = update_config {
        tracing::warn!(%error, "Could not update folder bindings after project change");
    }
    match new_name {
        Some(name) => {
            let mut renamed = project;
            renamed.name = name;
            Json(ProjectResponse::from(&renamed)).into_response()
        }
        None => StatusCode::NO_CONTENT.into_response(),
    }
}
