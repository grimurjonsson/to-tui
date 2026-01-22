use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::storage::database;

pub const DEFAULT_PROJECT_NAME: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            created_at: Utc::now(),
        }
    }

    pub fn default_project() -> Self {
        Self::new(DEFAULT_PROJECT_NAME)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProjectRegistry {
    pub projects: Vec<Project>,
}

impl ProjectRegistry {
    /// Load projects from the database
    pub fn load() -> Result<Self> {
        let projects = database::load_projects()?;
        Ok(Self { projects })
    }

    pub fn ensure_default_project(&mut self) -> Result<&Project> {
        if self.get_by_name(DEFAULT_PROJECT_NAME).is_none() {
            let project = database::ensure_default_project_exists()?;
            self.projects.push(project);
        }
        Ok(self
            .get_by_name(DEFAULT_PROJECT_NAME)
            .expect("Default project must exist"))
    }

    pub fn get_by_name(&self, name: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.name == name)
    }

    pub fn create(&mut self, name: impl Into<String>) -> Result<&Project> {
        let name = name.into();

        if self.get_by_name(&name).is_some() {
            anyhow::bail!("Project '{}' already exists", name);
        }

        let project = Project::new(name);
        database::create_project(&project)?;
        self.projects.push(project);

        Ok(self.projects.last().expect("Just pushed a project"))
    }

    pub fn rename(&mut self, old_name: &str, new_name: impl Into<String>) -> Result<()> {
        let new_name = new_name.into();

        if old_name == DEFAULT_PROJECT_NAME {
            anyhow::bail!("Cannot rename the default project");
        }

        if self.get_by_name(&new_name).is_some() {
            anyhow::bail!("Project '{}' already exists", new_name);
        }

        database::rename_project(old_name, &new_name)?;

        let project = self
            .projects
            .iter_mut()
            .find(|p| p.name == old_name)
            .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", old_name))?;

        project.name = new_name;

        Ok(())
    }

    pub fn delete(&mut self, name: &str) -> Result<()> {
        if name == DEFAULT_PROJECT_NAME {
            anyhow::bail!("Cannot delete the default project");
        }

        let index = self
            .projects
            .iter()
            .position(|p| p.name == name)
            .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", name))?;

        database::delete_project(name)?;
        self.projects.remove(index);

        Ok(())
    }

    /// Returns projects sorted alphabetically with "default" always first
    pub fn list_sorted(&self) -> Vec<&Project> {
        let mut projects: Vec<&Project> = self.projects.iter().collect();
        projects.sort_by(|a, b| {
            if a.name == DEFAULT_PROJECT_NAME {
                std::cmp::Ordering::Less
            } else if b.name == DEFAULT_PROJECT_NAME {
                std::cmp::Ordering::Greater
            } else {
                a.name.cmp(&b.name)
            }
        });
        projects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_new() {
        let project = Project::new("Work");
        assert_eq!(project.name, "Work");
        assert!(!project.id.is_nil());
    }

    #[test]
    fn test_project_default() {
        let project = Project::default_project();
        assert_eq!(project.name, DEFAULT_PROJECT_NAME);
    }

    #[test]
    fn test_registry_get_by_name() {
        let mut registry = ProjectRegistry::default();
        registry.projects.push(Project::new("Work"));
        registry.projects.push(Project::new("Home"));

        assert!(registry.get_by_name("Work").is_some());
        assert!(registry.get_by_name("Home").is_some());
        assert!(registry.get_by_name("NotFound").is_none());
    }

    #[test]
    fn test_registry_list_sorted() {
        let mut registry = ProjectRegistry::default();
        registry.projects.push(Project::default_project());
        registry.projects.push(Project::new("Zebra"));
        registry.projects.push(Project::new("Alpha"));
        registry.projects.push(Project::new("Beta"));

        let sorted = registry.list_sorted();
        assert_eq!(sorted[0].name, DEFAULT_PROJECT_NAME);
        assert_eq!(sorted[1].name, "Alpha");
        assert_eq!(sorted[2].name, "Beta");
        assert_eq!(sorted[3].name, "Zebra");
    }
}
