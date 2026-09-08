use base64::{Engine, engine::general_purpose::STANDARD};
use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

use std::io::{self, IsTerminal, Write};

const PNG: &[u8] = include_bytes!("../../assets/github.png");
#[cfg(unix)]
const QUERY: &[u8] = b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[5n";

#[derive(Debug, Clone)]
pub struct GithubIcon {
    id: u32,
}

impl GithubIcon {
    pub fn detect() -> Option<Self> {
        if !io::stdin().is_terminal()
            || !io::stdout().is_terminal()
            || std::env::var_os("TMUX").is_some()
            || std::env::var("TERM").is_ok_and(|term| term == "dumb" || term.starts_with("screen"))
        {
            return None;
        }
        let term = std::env::var("TERM").unwrap_or_default();
        let program = std::env::var("TERM_PROGRAM").unwrap_or_default();
        if !matches!(term.as_str(), "xterm-kitty" | "xterm-ghostty")
            && !program.eq_ignore_ascii_case("ghostty")
            && !program.eq_ignore_ascii_case("kitty")
        {
            return None;
        }
        if probe().unwrap_or(false) {
            let id = (uuid::Uuid::new_v4().as_u128() as u32 & 0x00ff_ffff).max(1);
            Some(Self { id })
        } else {
            None
        }
    }

    pub fn upload(&self, writer: &mut impl Write) -> io::Result<()> {
        let encoded = STANDARD.encode(PNG);
        let chunks: Vec<_> = encoded.as_bytes().chunks(4096).collect();
        for (index, chunk) in chunks.iter().enumerate() {
            let more = u8::from(index + 1 < chunks.len());
            if index == 0 {
                write!(
                    writer,
                    "\x1b_Ga=T,U=1,f=100,t=d,c=2,r=1,i={},q=2,m={more};",
                    self.id
                )?;
            } else {
                write!(writer, "\x1b_Gq=2,m={more};")?;
            }
            writer.write_all(chunk)?;
            writer.write_all(b"\x1b\\")?;
        }
        writer.flush()
    }

    pub fn delete(&self, writer: &mut impl Write) -> io::Result<()> {
        write!(writer, "\x1b_Ga=d,d=I,i={},q=2\x1b\\", self.id)
    }
}

impl Widget for &GithubIcon {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.width < 2 || area.height == 0 {
            return;
        }
        let [_, r, g, b] = self.id.to_be_bytes();
        for (index, column) in ['\u{0305}', '\u{030d}'].into_iter().enumerate() {
            if let Some(cell) = buffer.cell_mut((area.x + index as u16, area.y)) {
                cell.set_symbol(&format!("\u{10eeee}\u{0305}{column}"))
                    .set_fg(Color::Rgb(r, g, b));
            }
        }
    }
}

#[cfg(unix)]
fn probe() -> io::Result<bool> {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use std::time::{Duration, Instant};

    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    stdout.write_all(QUERY)?;
    stdout.flush()?;
    let deadline = Instant::now() + Duration::from_millis(250);
    let mut response = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || response.len() >= 4096 {
            return Ok(false);
        }
        let mut poll = libc::pollfd {
            fd: stdin.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut poll, 1, remaining.as_millis().max(1) as i32) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if result == 0 {
            return Ok(false);
        }
        let mut bytes = [0; 256];
        let count = stdin.read(&mut bytes)?;
        if count == 0 {
            return Ok(false);
        }
        response.extend_from_slice(&bytes[..count]);
        if response.windows(4).any(|bytes| bytes == b"\x1b[0n") {
            return Ok(response
                .windows(12)
                .any(|bytes| bytes == b"\x1b_Gi=31;OK\x1b\\"));
        }
    }
}

#[cfg(not(unix))]
fn probe() -> io::Result<bool> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_embeds_png_in_bounded_chunks_and_deletes_only_own_image() {
        let icon = GithubIcon { id: 42 };
        let mut output = Vec::new();
        icon.upload(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.starts_with("\x1b_Ga=T,U=1,f=100,t=d,c=2,r=1,i=42,q=2,"));
        let mut payload = String::new();
        for command in output.split("\x1b\\").filter(|part| !part.is_empty()) {
            let (_, data) = command.split_once(';').unwrap();
            assert!(data.len() <= 4096);
            payload.push_str(data);
        }
        assert_eq!(STANDARD.decode(payload).unwrap(), PNG);
        let mut deleted = Vec::new();
        icon.delete(&mut deleted).unwrap();
        assert_eq!(deleted, b"\x1b_Ga=d,d=I,i=42,q=2\x1b\\");
    }
}
