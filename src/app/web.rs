use crate::cli::DEFAULT_API_PORT;
use crate::web_process::{self, WebStatus};

use anyhow::{Context, Result, bail};

use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebAction {
    Start,
    Stop,
    Restart,
    Open,
}

impl WebAction {
    pub const ALL: [Self; 4] = [Self::Start, Self::Stop, Self::Restart, Self::Open];

    pub fn title(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Restart => "Restart",
            Self::Open => "Open in browser",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Start => "starting",
            Self::Stop => "stopping",
            Self::Restart => "restarting",
            Self::Open => "opening",
        }
    }
}

#[derive(Debug)]
struct Update {
    status: Result<WebStatus, String>,
    operation_error: Option<String>,
    completed: bool,
}

#[derive(Debug)]
pub struct WebController {
    pub status: Option<WebStatus>,
    pub error: Option<String>,
    pub port: u16,
    selected: usize,
    busy: Option<WebAction>,
    pending: Option<WebAction>,
    receiver: Option<Receiver<Update>>,
    last_check: Option<Instant>,
}

impl Default for WebController {
    fn default() -> Self {
        Self {
            status: None,
            error: None,
            port: DEFAULT_API_PORT,
            selected: 0,
            busy: None,
            pending: None,
            receiver: None,
            last_check: None,
        }
    }
}

impl WebController {
    pub fn selected_action(&self) -> WebAction {
        WebAction::ALL[self.selected]
    }

    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1) % WebAction::ALL.len();
    }

    pub fn select_previous(&mut self) {
        self.selected = (self.selected + WebAction::ALL.len() - 1) % WebAction::ALL.len();
    }

    pub fn activate_selected(&mut self) {
        self.request(self.selected_action());
    }

    pub fn label(&self) -> &str {
        if let Some(action) = self.busy {
            return action.label();
        }
        match &self.status {
            Some(WebStatus::Running { .. }) => "running",
            Some(WebStatus::Stopped) => "stopped",
            Some(WebStatus::External) => "external",
            None if self.error.is_some() => "error",
            None => "checking",
        }
    }

    pub fn request(&mut self, action: WebAction) {
        if self.busy.is_some() {
            return;
        }
        self.error = None;
        self.pending = Some(action);
        self.busy = Some(action);
    }

    pub fn tick(&mut self) {
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(update) => {
                    self.receiver = None;
                    if update.completed {
                        self.busy = None;
                        self.error = update.operation_error;
                    }
                    match update.status {
                        Ok(status) => {
                            if let WebStatus::Running { url, .. } = &status
                                && let Ok(url) = reqwest::Url::parse(url)
                                && let Some(port) = url.port_or_known_default()
                            {
                                self.port = port;
                            }
                            self.status = Some(status);
                        }
                        Err(error) => {
                            self.status = None;
                            self.error = Some(error);
                        }
                    }
                    self.last_check = Some(Instant::now());
                }
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.busy = None;
                    self.error = Some("Web process worker exited unexpectedly".into());
                    self.last_check = Some(Instant::now());
                }
            }
        }
        if self.pending.is_none()
            && self
                .last_check
                .is_some_and(|time| time.elapsed() < Duration::from_secs(2))
        {
            return;
        }
        let action = self.pending.take();
        let port = self.port;
        let url = match &self.status {
            Some(WebStatus::Running { url, .. }) => Some(url.clone()),
            _ => None,
        };
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        std::thread::spawn(move || {
            let operation_error = action
                .and_then(|action| execute(action, port, url).err())
                .map(|error| format!("{error:#}"));
            let status = web_process::status(port).map_err(|error| format!("{error:#}"));
            let _ = sender.send(Update {
                status,
                operation_error,
                completed: action.is_some(),
            });
        });
    }
}

fn execute(action: WebAction, port: u16, url: Option<String>) -> Result<()> {
    let verb = match action {
        WebAction::Start => "start",
        WebAction::Stop => "stop",
        WebAction::Restart => "restart",
        WebAction::Open => {
            open::that(url.context("Start the web server before opening the browser")?)?;
            return Ok(());
        }
    };
    let output = Command::new(std::env::current_exe()?)
        .args(["web", verb, "--port", &port.to_string()])
        .output()
        .context("Failed to run web command")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_refresh_preserves_queued_stop() {
        let mut controller = WebController::default();
        let (sender, receiver) = mpsc::channel();
        controller.receiver = Some(receiver);
        controller.request(WebAction::Stop);
        controller.request(WebAction::Start);
        controller.tick();
        assert!(matches!(controller.pending, Some(WebAction::Stop)));
        assert_eq!(controller.label(), "stopping");
        drop(sender);
    }
}
