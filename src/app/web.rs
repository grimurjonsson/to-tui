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
    remote_url: Option<String>,
    selected: usize,
    busy: Option<WebAction>,
    pending: Option<(WebAction, String)>,
    receiver: Option<Receiver<Update>>,
    last_check: Option<Instant>,
}

impl Default for WebController {
    fn default() -> Self {
        Self {
            status: None,
            error: None,
            port: DEFAULT_API_PORT,
            remote_url: to_tui::remote::active().map(|client| client.url().to_owned()),
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
        if self.is_remote() {
            WebAction::Open
        } else {
            WebAction::ALL[self.selected]
        }
    }

    pub fn is_remote(&self) -> bool {
        self.remote_url.is_some()
    }

    pub fn action_enabled(&self, action: WebAction) -> bool {
        !self.is_remote() || action == WebAction::Open
    }

    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1) % WebAction::ALL.len();
    }

    pub fn select_previous(&mut self) {
        self.selected = (self.selected + WebAction::ALL.len() - 1) % WebAction::ALL.len();
    }

    pub fn activate_selected(&mut self, project: &str) {
        self.request(self.selected_action(), project);
    }

    pub fn label(&self) -> &str {
        if let Some(url) = &self.remote_url {
            return url;
        }
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

    pub fn request(&mut self, action: WebAction, project: &str) {
        if let Some(url) = &self.remote_url {
            if action == WebAction::Open {
                self.error = open_project(url, project).err().map(|e| e.to_string());
            } else {
                self.error = Some("Manage the remote service on its server".into());
            }
            return;
        }
        if self.busy.is_some() {
            return;
        }
        self.error = None;
        self.pending = Some((action, project.to_owned()));
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
        let pending = self.pending.take();
        let port = self.port;
        let url = match &self.status {
            Some(WebStatus::Running { url, .. }) => Some(url.clone()),
            _ => None,
        };
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        std::thread::spawn(move || {
            let completed = pending.is_some();
            let operation_error = pending
                .and_then(|(action, project)| execute(action, port, url, &project).err())
                .map(|error| format!("{error:#}"));
            let status = web_process::status(port).map_err(|error| format!("{error:#}"));
            let _ = sender.send(Update {
                status,
                operation_error,
                completed,
            });
        });
    }
}

fn project_url(base: &str, project: &str) -> Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(base).context("Invalid web server URL")?;
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| key != "project")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.query_pairs_mut()
        .clear()
        .extend_pairs(pairs)
        .append_pair("project", project);
    Ok(url)
}

fn open_project(base: &str, project: &str) -> Result<()> {
    open::that(project_url(base, project)?.as_str())?;
    Ok(())
}

fn execute(action: WebAction, port: u16, url: Option<String>, project: &str) -> Result<()> {
    let verb = match action {
        WebAction::Start => "start",
        WebAction::Stop => "stop",
        WebAction::Restart => "restart",
        WebAction::Open => {
            open_project(
                &url.context("Start the web server before opening the browser")?,
                project,
            )?;
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
    fn test_browser_url_selects_current_project_for_local_and_remote_servers() {
        for base in [
            "http://127.0.0.1:48372/?project=old",
            "https://todo.example.com/?project=old&view=list",
        ] {
            let url = project_url(base, "Notes & ideas #1").unwrap();
            let projects: Vec<_> = url
                .query_pairs()
                .filter(|(key, _)| key == "project")
                .map(|(_, value)| value.into_owned())
                .collect();
            assert_eq!(projects, ["Notes & ideas #1"]);
            assert!(url.fragment().is_none());
            if base.contains("view=list") {
                assert!(
                    url.query_pairs()
                        .any(|(key, value)| key == "view" && value == "list")
                );
            }
        }
    }

    #[test]
    fn test_remote_web_navigation_only_selects_open() {
        let mut controller = WebController {
            remote_url: Some("https://todo.example.com".into()),
            ..WebController::default()
        };
        assert_eq!(controller.label(), "https://todo.example.com");
        assert_eq!(controller.selected_action(), WebAction::Open);
        for _ in 0..WebAction::ALL.len() {
            controller.select_next();
            assert_eq!(controller.selected_action(), WebAction::Open);
        }
        for _ in 0..WebAction::ALL.len() {
            controller.select_previous();
            assert_eq!(controller.selected_action(), WebAction::Open);
        }
        for action in [WebAction::Start, WebAction::Stop, WebAction::Restart] {
            assert!(!controller.action_enabled(action));
            controller.request(action, "default");
            assert!(controller.pending.is_none());
            assert!(controller.busy.is_none());
        }
        assert!(controller.action_enabled(WebAction::Open));
    }

    #[test]
    fn test_web_refresh_preserves_queued_stop() {
        let mut controller = WebController::default();
        let (sender, receiver) = mpsc::channel();
        controller.receiver = Some(receiver);
        controller.request(WebAction::Stop, "default");
        controller.request(WebAction::Start, "default");
        controller.tick();
        assert!(matches!(controller.pending, Some((WebAction::Stop, _))));
        assert_eq!(controller.label(), "stopping");
        drop(sender);
    }
}
