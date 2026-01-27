//! Plugin installer for local and remote plugin installation.
//!
//! This module handles installing plugins from local directories or remote sources.
//! Local installs copy files (no symlinks) to the plugins directory.

use crate::plugin::manager::PluginManager;
use crate::plugin::marketplace::fetch_marketplace;
use crate::utils::paths::get_plugins_dir;
use crate::utils::upgrade::get_target_triple;
use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use std::fs;
use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::tempdir;
use totui_plugin_interface::{is_version_compatible, INTERFACE_VERSION};

/// Result of a successful plugin installation.
#[derive(Debug, Clone)]
pub struct InstallResult {
    /// Name of the installed plugin
    pub plugin_name: String,
    /// Version of the installed plugin
    pub version: String,
    /// Path where the plugin was installed
    pub path: PathBuf,
}

/// Parsed plugin source for installation.
///
/// Can represent either a local directory path or a remote GitHub source.
#[derive(Debug, Clone)]
pub struct PluginSource {
    /// GitHub owner (for remote sources)
    pub owner: Option<String>,
    /// GitHub repository (for remote sources)
    pub repo: Option<String>,
    /// Plugin name
    pub plugin_name: String,
    /// Version to install (None = latest)
    pub version: Option<String>,
    /// Local filesystem path (for local installs)
    pub local_path: Option<PathBuf>,
}

impl PluginSource {
    /// Parse a plugin source string.
    ///
    /// Detects local paths vs remote format (owner/repo/plugin-name).
    /// Local paths are detected by:
    /// - Starting with "/" (absolute path)
    /// - Starting with "./" or "../" (relative path)
    /// - Starting with "~" (home directory)
    /// - Existing as a directory on the filesystem
    ///
    /// # Examples
    /// ```ignore
    /// // Local paths
    /// PluginSource::parse("/path/to/plugin");
    /// PluginSource::parse("./my-plugin");
    /// PluginSource::parse("~/plugins/my-plugin");
    ///
    /// // Remote format (future support)
    /// PluginSource::parse("grimurjonsson/to-tui-plugins/jira");
    /// ```
    pub fn parse(source: &str) -> Result<Self> {
        // Check if this looks like a local path
        if source.starts_with('/')
            || source.starts_with("./")
            || source.starts_with("../")
            || source.starts_with('~')
        {
            return Self::parse_local(source);
        }

        // Check if path exists on filesystem (handles relative paths without ./)
        let path = Path::new(source);
        if path.exists() && path.is_dir() {
            return Self::parse_local(source);
        }

        // Try to parse as remote format: owner/repo/plugin-name
        let parts: Vec<&str> = source.split('/').collect();
        if parts.len() == 3 {
            Ok(Self {
                owner: Some(parts[0].to_string()),
                repo: Some(parts[1].to_string()),
                plugin_name: parts[2].to_string(),
                version: None,
                local_path: None,
            })
        } else {
            bail!(
                "Invalid plugin source format.\n\
                 Expected: local path or owner/repo/plugin-name\n\
                 Examples:\n\
                   totui plugin install /path/to/my-plugin\n\
                   totui plugin install grimurjonsson/to-tui-plugins/jira"
            );
        }
    }

    /// Parse a local filesystem path.
    fn parse_local(source: &str) -> Result<Self> {
        let expanded = if source.starts_with('~') {
            let home = dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
            home.join(&source[2..])
        } else {
            PathBuf::from(source)
        };

        let canonical = expanded
            .canonicalize()
            .with_context(|| format!("Path does not exist: {}", source))?;

        // Extract plugin name from directory name
        let plugin_name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid directory name"))?
            .to_string();

        Ok(Self {
            owner: None,
            repo: None,
            plugin_name,
            version: None,
            local_path: Some(canonical),
        })
    }

    /// Check if this is a local install source.
    pub fn is_local(&self) -> bool {
        self.local_path.is_some()
    }
}

/// Plugin installer for local and remote installations.
pub struct PluginInstaller;

impl PluginInstaller {
    /// Install a plugin from a local directory.
    ///
    /// This method:
    /// 1. Validates the source directory contains plugin.toml
    /// 2. Loads and validates the manifest
    /// 3. Checks version compatibility (min_interface_version)
    /// 4. Checks for existing installation (requires force=true to overwrite)
    /// 5. Copies the entire directory to the plugins folder
    ///
    /// # Arguments
    /// * `source_dir` - Path to the plugin directory
    /// * `force` - If true, overwrite existing installation
    ///
    /// # Returns
    /// * `InstallResult` with plugin name, version, and installed path
    pub fn install_from_local(source_dir: &Path, force: bool) -> Result<InstallResult> {
        // 1. Check source directory exists and is a directory
        if !source_dir.exists() {
            bail!("Source directory does not exist: {:?}", source_dir);
        }
        if !source_dir.is_dir() {
            bail!("Source path is not a directory: {:?}", source_dir);
        }

        // 2. Check source directory contains plugin.toml
        let manifest_path = source_dir.join("plugin.toml");
        if !manifest_path.exists() {
            bail!(
                "Source directory does not contain plugin.toml: {:?}\n\
                 A valid plugin directory must contain a plugin.toml manifest file.",
                source_dir
            );
        }

        // 3. Load and validate manifest using PluginManager::load_plugin_info pattern
        let info = PluginManager::load_plugin_info(source_dir);

        // Check for parse/validation errors
        if let Some(ref error) = info.error {
            bail!("Invalid plugin manifest: {}", error);
        }

        let plugin_name = &info.manifest.name;
        let version = &info.manifest.version;

        // 4. Check version compatibility (min_interface_version)
        if let Some(ref min_ver) = info.manifest.min_interface_version {
            match is_version_compatible(min_ver, INTERFACE_VERSION) {
                Ok(true) => {
                    // Compatible - continue
                }
                Ok(false) => {
                    bail!(
                        "Plugin '{}' v{} is not compatible with this version of to-tui.\n\
                         Plugin requires interface version {}, but host provides {}.\n\
                         Please upgrade to-tui or use an older version of this plugin.",
                        plugin_name,
                        version,
                        min_ver,
                        INTERFACE_VERSION
                    );
                }
                Err(e) => {
                    bail!("Failed to check version compatibility: {}", e);
                }
            }
        }

        // 5. Get plugins directory and target path
        let plugins_dir = get_plugins_dir()?;

        // Ensure plugins directory exists
        if !plugins_dir.exists() {
            fs::create_dir_all(&plugins_dir)
                .with_context(|| format!("Failed to create plugins directory: {:?}", plugins_dir))?;
        }

        let target_dir = plugins_dir.join(plugin_name);

        // 6. Check for existing installation
        if target_dir.exists() {
            if force {
                // Remove existing installation
                fs::remove_dir_all(&target_dir).with_context(|| {
                    format!(
                        "Failed to remove existing plugin installation: {:?}",
                        target_dir
                    )
                })?;
            } else {
                bail!(
                    "Plugin '{}' is already installed at {:?}\n\
                     Use --force to overwrite the existing installation.",
                    plugin_name,
                    target_dir
                );
            }
        }

        // 7. Copy entire directory recursively
        copy_dir_recursive(source_dir, &target_dir).with_context(|| {
            format!(
                "Failed to copy plugin files from {:?} to {:?}",
                source_dir, target_dir
            )
        })?;

        // 8. Write source tracking file
        let source_file = target_dir.join(".source");
        fs::write(&source_file, "local").ok(); // Non-fatal if fails

        Ok(InstallResult {
            plugin_name: plugin_name.clone(),
            version: version.clone(),
            path: target_dir,
        })
    }

    /// Install a plugin from a remote GitHub release.
    ///
    /// This method:
    /// 1. Constructs the download URL from the source (owner/repo/plugin-name)
    /// 2. Downloads the tar.gz archive to a temp directory
    /// 3. Extracts and validates the plugin manifest
    /// 4. Checks for existing installation (requires force=true to overwrite)
    /// 5. Moves the extracted plugin to the plugins directory
    ///
    /// # Arguments
    /// * `source` - Parsed plugin source with owner, repo, plugin_name, and version
    /// * `force` - If true, overwrite existing installation
    ///
    /// # Returns
    /// * `InstallResult` with plugin name, version, and installed path
    pub fn install_from_remote(source: &PluginSource, force: bool) -> Result<InstallResult> {
        // 1. Version should be set (either by CLI arg or resolved from marketplace)
        let _version = source.version.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Version required. Use --version to specify or ensure marketplace is available.\n\
                 Example: totui plugin install grimurjonsson/to-tui-plugins/jira --version 1.0.0"
            )
        })?;

        // 2. Construct download URL
        let url = get_plugin_download_url(source)?;
        println!("Downloading from: {}", url);

        // 3. Download to temp directory
        let temp_dir = tempdir().context("Failed to create temp directory")?;
        let archive_path = temp_dir.path().join("plugin.tar.gz");

        println!("Downloading...");
        download_plugin_blocking(&url, &archive_path)?;
        println!("Download complete.");

        // 4. Extract archive
        println!("Extracting...");
        let extracted_dir = extract_plugin_archive(&archive_path, temp_dir.path())?;
        println!("Extraction complete.");

        // 5. Validate manifest
        println!("Verifying...");
        let info = PluginManager::load_plugin_info(&extracted_dir);
        if let Some(err) = &info.error {
            bail!("Invalid plugin: {}", err);
        }
        if !info.available
            && let Some(reason) = &info.availability_reason
        {
            bail!("Plugin not compatible: {}", reason);
        }

        // 6. Check for existing installation
        let plugins_dir = get_plugins_dir()?;
        let target_dir = plugins_dir.join(&source.plugin_name);
        if target_dir.exists() && !force {
            bail!(
                "Plugin '{}' already installed at {:?}. Use --force to overwrite.",
                source.plugin_name,
                target_dir
            );
        }

        // 7. Move to plugins directory
        println!("Installing...");
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir).context("Failed to remove existing plugin")?;
        }
        fs::create_dir_all(&plugins_dir)?;

        // Try rename first, fall back to copy (rename fails across filesystems)
        fs::rename(&extracted_dir, &target_dir).or_else(|_| {
            copy_dir_recursive(&extracted_dir, &target_dir)?;
            fs::remove_dir_all(&extracted_dir)?;
            Ok::<(), anyhow::Error>(())
        })?;

        // Write source tracking file
        let source_content = format!(
            "{}/{}",
            source.owner.as_ref().unwrap(),
            source.repo.as_ref().unwrap()
        );
        let source_file = target_dir.join(".source");
        fs::write(&source_file, &source_content).ok(); // Non-fatal if fails

        println!("Done!");

        Ok(InstallResult {
            plugin_name: info.manifest.name,
            version: info.manifest.version,
            path: target_dir,
        })
    }

    /// Resolve the latest version for a plugin from its marketplace.
    ///
    /// Fetches the marketplace manifest and looks up the plugin's latest version.
    pub fn resolve_latest_version(source: &PluginSource) -> Result<String> {
        let owner = source
            .owner
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Cannot resolve version for local install"))?;
        let repo = source
            .repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Cannot resolve version for local install"))?;

        println!("Fetching marketplace manifest...");
        let manifest = fetch_marketplace(owner, repo)?;

        let entry = manifest.find_plugin(&source.plugin_name).ok_or_else(|| {
            anyhow::anyhow!(
                "Plugin '{}' not found in marketplace {}/{}.\n\
                 Check the marketplace for available plugins.",
                source.plugin_name,
                owner,
                repo
            )
        })?;

        Ok(entry.version.clone())
    }
}

/// Constructs the download URL for a plugin release.
///
/// URL format: https://github.com/{owner}/{repo}/releases/download/v{version}/{plugin}-{target}.tar.gz
fn get_plugin_download_url(source: &PluginSource) -> Result<String> {
    let target = get_target_triple();
    let version = source
        .version
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Version required for remote install"))?;
    let owner = source
        .owner
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Owner required for remote install"))?;
    let repo = source
        .repo
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Repo required for remote install"))?;

    Ok(format!(
        "https://github.com/{}/{}/releases/download/v{}/{}-{}.tar.gz",
        owner, repo, version, source.plugin_name, target
    ))
}

/// Download plugin archive (blocking, simple implementation).
fn download_plugin_blocking(url: &str, target_path: &Path) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("to-tui")
        .build()?;

    let response = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .send()?;

    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        let target = get_target_triple();
        bail!(
            "Plugin binary not found for platform '{}'.\n\
             The plugin may not be built for your platform.\n\
             Check the release page for available platforms.",
            target
        );
    }
    if !status.is_success() {
        bail!(
            "HTTP error: {} {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown")
        );
    }

    let bytes = response.bytes()?;
    fs::write(target_path, &bytes)?;
    Ok(())
}

/// Extract tar.gz archive to target directory.
///
/// Returns the path to the extracted plugin directory.
/// Handles archives that may have a single nested directory.
fn extract_plugin_archive(archive_path: &Path, target_dir: &Path) -> Result<PathBuf> {
    let tar_gz = fs::File::open(archive_path).context("Failed to open archive")?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);

    let extracted_dir = target_dir.join("extracted");
    fs::create_dir_all(&extracted_dir)?;
    archive
        .unpack(&extracted_dir)
        .context("Failed to extract archive")?;

    // Find the plugin directory (may be nested one level)
    let entries: Vec<_> = fs::read_dir(&extracted_dir)?
        .filter_map(|e| e.ok())
        .collect();

    // If there's exactly one directory, use that as the plugin root
    if entries.len() == 1 && entries[0].path().is_dir() {
        Ok(entries[0].path())
    } else {
        Ok(extracted_dir)
    }
}

/// Recursively copy a directory and all its contents.
fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    // Create target directory
    fs::create_dir_all(target)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path)?;
        }
        // Skip symlinks and other file types
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_plugin(dir: &Path, name: &str, version: &str) -> PathBuf {
        let plugin_dir = dir.join(name);
        fs::create_dir_all(&plugin_dir).unwrap();

        let manifest = format!(
            r#"name = "{}"
version = "{}"
description = "Test plugin"
"#,
            name, version
        );

        let manifest_path = plugin_dir.join("plugin.toml");
        let mut file = fs::File::create(&manifest_path).unwrap();
        file.write_all(manifest.as_bytes()).unwrap();

        plugin_dir
    }

    #[test]
    fn test_plugin_source_parse_local_absolute() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = create_test_plugin(temp_dir.path(), "my-plugin", "1.0.0");

        let source = PluginSource::parse(plugin_dir.to_str().unwrap()).unwrap();
        assert!(source.is_local());
        assert_eq!(source.plugin_name, "my-plugin");
        assert!(source.local_path.is_some());
    }

    #[test]
    fn test_plugin_source_parse_remote_format() {
        let source = PluginSource::parse("grimurjonsson/to-tui-plugins/jira").unwrap();
        assert!(!source.is_local());
        assert_eq!(source.owner, Some("grimurjonsson".to_string()));
        assert_eq!(source.repo, Some("to-tui-plugins".to_string()));
        assert_eq!(source.plugin_name, "jira");
    }

    #[test]
    fn test_plugin_source_parse_invalid() {
        // Invalid format (only 2 parts)
        let result = PluginSource::parse("owner/repo");
        assert!(result.is_err());

        // Non-existent path that doesn't match remote format
        let result = PluginSource::parse("single-word-not-a-path");
        assert!(result.is_err());
    }

    #[test]
    fn test_install_missing_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("no-manifest");
        fs::create_dir_all(&plugin_dir).unwrap();

        let result = PluginInstaller::install_from_local(&plugin_dir, false);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("does not contain plugin.toml"));
    }

    #[test]
    fn test_install_invalid_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("bad-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();

        let manifest_path = plugin_dir.join("plugin.toml");
        fs::write(&manifest_path, "invalid toml [[[").unwrap();

        let result = PluginInstaller::install_from_local(&plugin_dir, false);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid plugin manifest"));
    }

    #[test]
    fn test_copy_dir_recursive() {
        let source_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();

        // Create some files and subdirectories
        fs::write(source_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(source_dir.path().join("file2.txt"), "content2").unwrap();

        let sub_dir = source_dir.path().join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("nested.txt"), "nested content").unwrap();

        // Copy
        let target = target_dir.path().join("copied");
        copy_dir_recursive(source_dir.path(), &target).unwrap();

        // Verify
        assert!(target.join("file1.txt").exists());
        assert!(target.join("file2.txt").exists());
        assert!(target.join("subdir").join("nested.txt").exists());

        assert_eq!(
            fs::read_to_string(target.join("file1.txt")).unwrap(),
            "content1"
        );
        assert_eq!(
            fs::read_to_string(target.join("subdir").join("nested.txt")).unwrap(),
            "nested content"
        );
    }
}
