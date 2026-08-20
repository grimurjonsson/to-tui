use std::collections::BTreeMap;
use std::path::Path;

use super::DEFAULT_PROJECT_NAME;

/// Compute the folder key for a directory: the enclosing git repository root
/// if the directory is inside one, otherwise the canonicalized directory itself.
/// Returns None if the directory cannot be resolved (e.g. it no longer exists).
pub fn folder_key_for(dir: &Path) -> Option<String> {
    let canonical = dir.canonicalize().ok()?;
    for ancestor in canonical.ancestors() {
        // A .git entry can be a directory or, in worktrees, a file
        if ancestor.join(".git").exists() {
            return Some(ancestor.to_string_lossy().into_owned());
        }
    }
    Some(canonical.to_string_lossy().into_owned())
}

/// Folder key for the process working directory.
pub fn current_folder_key() -> Option<String> {
    folder_key_for(&std::env::current_dir().ok()?)
}

/// Resolve which project to open: the folder's mapped project if it still
/// exists, else the last used project if it still exists, else the default.
pub fn resolve_project_name(
    folder_key: Option<&str>,
    folder_projects: &BTreeMap<String, String>,
    last_used_project: Option<&str>,
    project_exists: impl Fn(&str) -> bool,
) -> String {
    if let Some(key) = folder_key
        && let Some(name) = folder_projects.get(key)
        && project_exists(name)
    {
        return name.clone();
    }

    if let Some(name) = last_used_project
        && project_exists(name)
    {
        return name.to_string();
    }

    DEFAULT_PROJECT_NAME.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_resolve_prefers_folder_mapping() {
        let name = resolve_project_name(
            Some("/repo/a"),
            &map(&[("/repo/a", "proj-a")]),
            Some("other"),
            |_| true,
        );
        assert_eq!(name, "proj-a");
    }

    #[test]
    fn test_resolve_skips_mapping_to_missing_project() {
        let name = resolve_project_name(
            Some("/repo/a"),
            &map(&[("/repo/a", "deleted")]),
            Some("fallback"),
            |n| n != "deleted",
        );
        assert_eq!(name, "fallback");
    }

    #[test]
    fn test_resolve_unmapped_folder_uses_last_used() {
        let name = resolve_project_name(
            Some("/repo/b"),
            &map(&[("/repo/a", "proj-a")]),
            Some("recent"),
            |_| true,
        );
        assert_eq!(name, "recent");
    }

    #[test]
    fn test_resolve_no_folder_key_uses_last_used() {
        let name = resolve_project_name(None, &map(&[]), Some("recent"), |_| true);
        assert_eq!(name, "recent");
    }

    #[test]
    fn test_resolve_falls_back_to_default() {
        let name = resolve_project_name(Some("/repo/a"), &map(&[]), Some("gone"), |n| {
            n == DEFAULT_PROJECT_NAME
        });
        assert_eq!(name, DEFAULT_PROJECT_NAME);
    }

    #[test]
    fn test_folder_key_without_git_is_the_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();

        let key = folder_key_for(&sub).unwrap();
        assert_eq!(key, sub.canonicalize().unwrap().to_string_lossy());
    }

    #[test]
    fn test_folder_key_uses_git_root_from_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        let sub = tmp.path().join("src").join("deep");
        fs::create_dir_all(&sub).unwrap();

        let key = folder_key_for(&sub).unwrap();
        assert_eq!(key, tmp.path().canonicalize().unwrap().to_string_lossy());
    }

    #[test]
    fn test_folder_key_treats_git_file_as_root_marker() {
        // git worktrees have a .git *file* at the root instead of a directory
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".git"), "gitdir: /elsewhere").unwrap();
        let sub = tmp.path().join("src");
        fs::create_dir(&sub).unwrap();

        let key = folder_key_for(&sub).unwrap();
        assert_eq!(key, tmp.path().canonicalize().unwrap().to_string_lossy());
    }

    #[test]
    fn test_folder_key_missing_directory_is_none() {
        assert_eq!(folder_key_for(Path::new("/nonexistent/path/xyz")), None);
    }
}
