use anyhow::{Context, Result};
use arboard::Clipboard;
use std::sync::Mutex;
use std::path::PathBuf;

/// Internal yank buffer for headless environments
static YANK_BUFFER: Mutex<Option<String>> = Mutex::new(None);

/// Result of a copy operation
#[derive(Debug)]
pub enum CopyResult {
    /// Successfully copied to system clipboard
    SystemClipboard,
    /// System clipboard unavailable, saved to internal buffer (and optionally file)
    InternalBuffer { file_path: Option<PathBuf> },
}

/// Copy text to the system clipboard, with fallback for headless environments.
///
/// On systems with a display server (X11/Wayland/macOS/Windows), copies to
/// the system clipboard. On headless systems, saves to an internal buffer
/// and optionally to ~/.to-tui/yank.txt for retrieval via other means.
///
/// Returns the method used (system clipboard or internal buffer).
pub fn copy_to_clipboard(text: &str) -> Result<CopyResult> {
    // Try system clipboard first
    match Clipboard::new() {
        Ok(mut clipboard) => {
            match clipboard.set_text(text) {
                Ok(()) => {
                    // Also update internal buffer for consistency
                    if let Ok(mut buffer) = YANK_BUFFER.lock() {
                        *buffer = Some(text.to_string());
                    }
                    return Ok(CopyResult::SystemClipboard);
                }
                Err(e) => {
                    tracing::debug!("System clipboard set_text failed: {}", e);
                    // Fall through to internal buffer
                }
            }
        }
        Err(e) => {
            tracing::debug!("System clipboard unavailable: {}", e);
            // Fall through to internal buffer
        }
    }

    // Fallback: internal buffer + file
    if let Ok(mut buffer) = YANK_BUFFER.lock() {
        *buffer = Some(text.to_string());
    }

    // Also save to file for external access
    let file_path = save_to_yank_file(text);

    Ok(CopyResult::InternalBuffer { file_path })
}

/// Get text from internal yank buffer (for paste fallback)
pub fn get_from_internal_buffer() -> Option<String> {
    YANK_BUFFER.lock().ok().and_then(|b| b.clone())
}

/// Save yanked text to ~/.to-tui/yank.txt for retrieval
fn save_to_yank_file(text: &str) -> Option<PathBuf> {
    let path = crate::utils::paths::get_to_tui_dir()
        .ok()
        .map(|dir| dir.join("yank.txt"))?;
    
    match std::fs::write(&path, text) {
        Ok(()) => {
            tracing::debug!("Saved yank to {}", path.display());
            Some(path)
        }
        Err(e) => {
            tracing::warn!("Failed to save yank file: {}", e);
            None
        }
    }
}

/// Paste from clipboard with fallback to internal buffer
pub fn paste_from_clipboard() -> Result<String> {
    // Try system clipboard first
    if let Ok(mut clipboard) = Clipboard::new() {
        if let Ok(text) = clipboard.get_text() {
            return Ok(text);
        }
    }

    // Fallback: internal buffer
    get_from_internal_buffer()
        .context("No text in clipboard or internal buffer")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_internal_buffer_fallback() {
        // Clear any existing buffer
        if let Ok(mut buffer) = YANK_BUFFER.lock() {
            *buffer = None;
        }

        // On headless systems (like CI), this should fall back to internal buffer
        let result = copy_to_clipboard("test text");
        assert!(result.is_ok(), "copy_to_clipboard should not fail");

        // Verify internal buffer was populated
        let buffer_content = get_from_internal_buffer();
        assert_eq!(buffer_content, Some("test text".to_string()));
    }

    #[test]
    fn test_paste_from_internal_buffer() {
        // Set up internal buffer
        if let Ok(mut buffer) = YANK_BUFFER.lock() {
            *buffer = Some("buffered text".to_string());
        }

        // On headless systems, paste should use internal buffer
        // (system clipboard will fail, but internal buffer should work)
        let result = get_from_internal_buffer();
        assert_eq!(result, Some("buffered text".to_string()));
    }

    #[test]
    fn test_copy_result_variants() {
        // Just verify the enum variants exist and are constructable
        let _sys = CopyResult::SystemClipboard;
        let _internal = CopyResult::InternalBuffer { file_path: None };
        let _with_path = CopyResult::InternalBuffer { 
            file_path: Some(PathBuf::from("/tmp/test.txt")) 
        };
    }
}
