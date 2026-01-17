use anyhow::{Context, Result};
use arboard::Clipboard;

/// Copy text to the system clipboard.
///
/// Returns Ok(()) on success, or an error if clipboard is unavailable.
/// On Linux, clipboard contents persist while the application is running.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new()
        .context("Failed to access system clipboard")?;
    clipboard
        .set_text(text)
        .context("Failed to copy text to clipboard")?;
    Ok(())
}
