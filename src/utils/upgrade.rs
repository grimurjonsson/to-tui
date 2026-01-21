use std::path::PathBuf;

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

const GITHUB_REPO: &str = "grimurjonsson/to-tui";

/// State of the upgrade workflow, used by the TUI to render appropriate UI.
#[derive(Debug, Clone)]
pub enum UpgradeSubState {
    /// Initial state, showing Y/N/S options
    Prompt,
    /// Download in progress, shows progress bar
    Downloading {
        progress: f64,
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
    },
    /// Download failed, show retry option
    Error { message: String },
    /// Download complete, ask to restart
    RestartPrompt { downloaded_path: PathBuf },
}

/// Progress messages sent through the download channel.
#[derive(Debug, Clone)]
pub enum DownloadProgress {
    /// Download progress update
    Progress { bytes: u64, total: Option<u64> },
    /// Download completed successfully
    Complete { path: PathBuf },
    /// Download failed with error
    Error { message: String },
}

/// Constructs the download URL for the release asset based on current platform.
///
/// # Arguments
/// * `version` - The version string without 'v' prefix (e.g., "0.3.1")
///
/// # Returns
/// The full URL to the tar.gz asset for the current platform.
///
/// # Panics
/// Panics on unsupported platforms (not macOS or Linux x86_64).
pub fn get_asset_download_url(version: &str) -> String {
    let target = get_target_triple();
    format!(
        "https://github.com/{}/releases/download/v{}/totui-{}.tar.gz",
        GITHUB_REPO, version, target
    )
}

/// Returns the target triple for the current platform.
fn get_target_triple() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64")
    )))]
    {
        panic!(
            "Unsupported platform: {} {}. \
             Supported platforms: macOS (ARM/Intel), Linux (x86_64)",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    }
}

/// Spawns an async download task and returns a channel receiver for progress updates.
///
/// The download task:
/// 1. Creates HTTP client with appropriate headers
/// 2. Streams the response body in chunks
/// 3. Writes chunks to the target file
/// 4. Sends progress updates (rate-limited to every ~100KB)
/// 5. Sends Complete or Error on finish
///
/// # Arguments
/// * `url` - The full URL to download from
/// * `target_path` - Where to save the downloaded file
///
/// # Returns
/// A receiver that will receive DownloadProgress messages.
pub fn spawn_download(url: String, target_path: PathBuf) -> mpsc::Receiver<DownloadProgress> {
    let (tx, rx) = mpsc::channel(32);

    tokio::spawn(async move {
        if let Err(e) = download_file(&url, &target_path, &tx).await {
            let _ = tx
                .send(DownloadProgress::Error {
                    message: e.to_string(),
                })
                .await;
        }
    });

    rx
}

/// Internal download implementation.
async fn download_file(
    url: &str,
    target_path: &PathBuf,
    tx: &mpsc::Sender<DownloadProgress>,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("to-tui")
        .build()?;

    let response = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .send()
        .await?;

    // Check for HTTP errors
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("HTTP error: {} {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown"));
    }

    let total_size = response.content_length();
    let mut stream = response.bytes_stream();

    let mut file = tokio::fs::File::create(target_path).await?;
    let mut downloaded: u64 = 0;
    let mut last_progress_at: u64 = 0;
    const PROGRESS_INTERVAL: u64 = 100 * 1024; // Send progress every 100KB

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        // Rate-limit progress updates to avoid overwhelming the channel
        if downloaded - last_progress_at >= PROGRESS_INTERVAL || downloaded == total_size.unwrap_or(0) {
            let _ = tx
                .send(DownloadProgress::Progress {
                    bytes: downloaded,
                    total: total_size,
                })
                .await;
            last_progress_at = downloaded;
        }
    }

    file.flush().await?;

    // Send final progress update if we haven't
    if downloaded != last_progress_at {
        let _ = tx
            .send(DownloadProgress::Progress {
                bytes: downloaded,
                total: total_size,
            })
            .await;
    }

    let _ = tx
        .send(DownloadProgress::Complete {
            path: target_path.clone(),
        })
        .await;

    Ok(())
}

/// Formats a byte count into a human-readable string.
///
/// # Examples
/// ```
/// use to_tui::utils::upgrade::format_bytes;
/// assert_eq!(format_bytes(0), "0 B");
/// assert_eq!(format_bytes(512), "512 B");
/// assert_eq!(format_bytes(1024), "1.0 KB");
/// assert_eq!(format_bytes(1536), "1.5 KB");
/// assert_eq!(format_bytes(1048576), "1.0 MB");
/// assert_eq!(format_bytes(1572864), "1.5 MB");
/// ```
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1572864), "1.5 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
    }

    #[test]
    fn test_get_asset_download_url() {
        let url = get_asset_download_url("0.3.1");
        assert!(url.starts_with("https://github.com/grimurjonsson/to-tui/releases/download/v0.3.1/totui-"));
        assert!(url.ends_with(".tar.gz"));
    }

    #[test]
    fn test_get_target_triple() {
        let target = get_target_triple();
        // Should be one of our supported targets
        assert!(
            target == "aarch64-apple-darwin"
                || target == "x86_64-apple-darwin"
                || target == "x86_64-unknown-linux-gnu"
        );
    }
}
