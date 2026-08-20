mod folder_map;
mod registry;

pub use folder_map::{current_folder_key, folder_key_for, resolve_project_name};
pub use registry::{DEFAULT_PROJECT_NAME, Project, ProjectRegistry};
