use std::io::{self, Write};

/// Set mouse cursor to pointer (hand icon) using OSC 22 escape sequence.
/// Works in modern terminals like Kitty, Foot, Xterm, Ghostty.
pub fn set_mouse_cursor_pointer() {
    let _ = io::stdout().write_all(b"\x1b]22;pointer\x1b\\");
    let _ = io::stdout().flush();
}

/// Reset mouse cursor to default using OSC 22 escape sequence.
pub fn set_mouse_cursor_default() {
    let _ = io::stdout().write_all(b"\x1b]22;default\x1b\\");
    let _ = io::stdout().flush();
}
