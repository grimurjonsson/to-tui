use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const GITHUB_REPO: &str = "grimurjonsson/to-tui";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CHECK_INTERVAL_SECS: u64 = 600; // Check every 10 minutes

/// Result of a version check
#[derive(Debug, Clone)]
pub struct VersionCheckResult {
    pub latest_version: String,
    pub is_newer: bool,
}

/// Spawns a background thread that periodically checks for new versions
/// and sends results through the provided channel.
pub fn spawn_version_checker() -> mpsc::Receiver<VersionCheckResult> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        // Initial check after a short delay (don't slow down startup)
        thread::sleep(Duration::from_secs(5));

        loop {
            if let Some(result) = check_latest_version()
                && result.is_newer
            {
                // Only send if there's a newer version
                let _ = tx.send(result);
            }

            // Wait before next check
            thread::sleep(Duration::from_secs(CHECK_INTERVAL_SECS));
        }
    });

    rx
}

/// Checks GitHub releases API for the latest version
fn check_latest_version() -> Option<VersionCheckResult> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );

    // Use blocking reqwest since we're in a background thread
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("to-tui")
        .build()
        .ok()?;

    let response: serde_json::Value = client.get(&url).send().ok()?.json().ok()?;

    let tag_name = response.get("tag_name")?.as_str()?;

    // Strip leading 'v' if present
    let latest_version = tag_name.strip_prefix('v').unwrap_or(tag_name);

    let is_newer = is_version_newer(latest_version, CURRENT_VERSION);

    Some(VersionCheckResult {
        latest_version: latest_version.to_string(),
        is_newer,
    })
}

/// Compares two semver version strings, returns true if `latest` is newer than `current`
fn is_version_newer(latest: &str, current: &str) -> bool {
    let parse_version = |v: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() >= 3 {
            Some((
                parts[0].parse().ok()?,
                parts[1].parse().ok()?,
                parts[2].parse().ok()?,
            ))
        } else if parts.len() == 2 {
            Some((parts[0].parse().ok()?, parts[1].parse().ok()?, 0))
        } else {
            None
        }
    };

    match (parse_version(latest), parse_version(current)) {
        (Some((l_maj, l_min, l_patch)), Some((c_maj, c_min, c_patch))) => {
            (l_maj, l_min, l_patch) > (c_maj, c_min, c_patch)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_version_newer("1.0.0", "0.9.0"));
        assert!(is_version_newer("0.10.0", "0.9.0"));
        assert!(is_version_newer("0.9.1", "0.9.0"));
        assert!(is_version_newer("2.0.0", "1.9.9"));

        assert!(!is_version_newer("0.9.0", "0.9.0"));
        assert!(!is_version_newer("0.8.0", "0.9.0"));
        assert!(!is_version_newer("0.9.0", "1.0.0"));
    }
}
