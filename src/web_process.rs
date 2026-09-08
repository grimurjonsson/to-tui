use crate::cli::{WebCommand, WebOptions};

use anyhow::{Result, bail};

#[derive(Debug, Clone)]
pub enum WebStatus {
    Stopped,
    Running { url: String, legacy: bool },
    External,
}

pub fn status(port: u16) -> Result<WebStatus> {
    #[cfg(unix)]
    return unix::status(port);
    #[cfg(not(unix))]
    {
        let _ = port;
        bail!("Web management requires macOS or Linux")
    }
}

pub fn run(options: WebOptions) -> Result<()> {
    if options.log && options.command.is_some() {
        bail!("Use --log on its own, or web logs");
    }
    if matches!(
        options.command,
        Some(WebCommand::Stop | WebCommand::Status | WebCommand::Logs { .. })
    ) && (options.detach || options.restart || options.open || options.verbose)
    {
        bail!("Start options cannot be used with stop, status, or logs");
    }
    if options.command.is_none() && !options.detach && !options.restart && !options.log {
        return crate::run_server_foreground(options.port, options.open, options.verbose);
    }
    #[cfg(unix)]
    return unix::manage(options);
    #[cfg(not(unix))]
    bail!("Managed web processes currently require macOS or Linux; use web for foreground mode")
}

#[cfg(unix)]
mod unix {
    use super::*;
    use to_tui::utils::paths::get_to_tui_dir;

    use anyhow::Context;
    use serde::{Deserialize, Serialize};

    use std::fs::{self, File};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Record {
        pid: u32,
        identity: String,
        url: String,
    }

    fn identity(pid: u32) -> Result<Option<String>> {
        let output = Command::new("ps")
            .args([
                "-p",
                &pid.to_string(),
                "-o",
                "pgid=",
                "-o",
                "lstart=",
                "-o",
                "stat=",
            ])
            .env("LC_ALL", "C")
            .output()
            .context("Failed to inspect the managed web process")?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut parts: Vec<_> = text.split_whitespace().collect();
        if !output.status.success()
            || parts.len() < 7
            || parts.last().is_some_and(|s| s.starts_with('Z'))
        {
            return Ok(None);
        }
        parts.pop();
        Ok(Some(parts.join(" ")))
    }

    fn active(path: &Path) -> Result<Option<Record>> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let record: Record =
            serde_json::from_slice(&bytes).context("Invalid web process record")?;
        if identity(record.pid)?.as_deref() == Some(&record.identity) {
            return Ok(Some(record));
        }
        fs::remove_file(path)?;
        Ok(None)
    }

    fn legacy_record() -> Result<Option<Record>> {
        let path = to_tui::utils::paths::get_pid_file_path()?;
        let pid = match fs::read_to_string(path) {
            Ok(text) => match text.trim().parse::<u32>() {
                Ok(pid) => pid,
                Err(_) => return Ok(None),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let Some(stamp) = identity(pid)? else {
            return Ok(None);
        };
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "command="])
            .output()?;
        let command = String::from_utf8_lossy(&output.stdout);
        let args: Vec<_> = command.split_whitespace().collect();
        let executable = args
            .first()
            .and_then(|arg| Path::new(arg).file_name())
            .and_then(|name| name.to_str());
        if !output.status.success()
            || !matches!(executable, Some("totui" | "to-tui"))
            || args.get(1..3) != Some(&["serve", "start"])
            || !args.contains(&"--daemon")
            || identity(pid)?.as_deref() != Some(&stamp)
        {
            return Ok(None);
        }
        let port = args
            .windows(2)
            .find(|pair| pair[0] == "--port")
            .and_then(|pair| pair[1].parse::<u16>().ok())
            .unwrap_or(crate::cli::DEFAULT_API_PORT);
        let host = std::env::var("TOTUI_BIND").unwrap_or_else(|_| "127.0.0.1".into());
        let ip = host
            .parse()
            .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
        let url = crate::workspace_url(std::net::SocketAddr::new(ip, port), "default")?.to_string();
        Ok(Some(Record {
            pid,
            identity: stamp,
            url,
        }))
    }

    pub(super) fn status(port: u16) -> Result<WebStatus> {
        let path = get_to_tui_dir()?.join("web/process.json");
        match fs::read(path) {
            Ok(bytes) => {
                let record: Record = serde_json::from_slice(&bytes)?;
                if identity(record.pid)?.as_deref() == Some(&record.identity) {
                    return Ok(WebStatus::Running {
                        url: record.url,
                        legacy: false,
                    });
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if let Some(record) = legacy_record()? {
            return Ok(WebStatus::Running {
                url: record.url,
                legacy: true,
            });
        }
        if crate::is_server_running(port) {
            return Ok(WebStatus::External);
        }
        Ok(WebStatus::Stopped)
    }

    fn stop(record: Option<Record>, path: &Path) -> Result<()> {
        let Some(record) = record else {
            println!("Not running");
            return Ok(());
        };
        if identity(record.pid)?.as_deref() == Some(&record.identity) {
            let result = unsafe { libc::kill(record.pid.try_into()?, libc::SIGTERM) };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error).context("Failed to stop web server");
                }
            }
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while identity(record.pid)?.as_deref() == Some(&record.identity) {
            if Instant::now() >= deadline {
                bail!("Web server has not stopped yet; retry web stop");
            }
            sleep(Duration::from_millis(50));
        }
        fs::remove_file(path)?;
        println!("Web server stopped");
        Ok(())
    }

    fn describe(record: &Record, log: &Path) {
        println!(
            "Running (PID {})\nWorkspace available at {}\nLog: {}",
            record.pid,
            record.url,
            log.display()
        );
    }

    fn print_logs(path: &Path, follow: bool) -> Result<()> {
        let mut identity = None;
        let mut offset = 0;
        let mut stdout = std::io::stdout().lock();
        loop {
            match File::open(path) {
                Ok(mut file) => {
                    let metadata = file.metadata()?;
                    let current = (metadata.dev(), metadata.ino());
                    if identity != Some(current) || metadata.len() < offset {
                        offset = 0;
                        identity = Some(current);
                    }
                    file.seek(SeekFrom::Start(offset))?;
                    let result = std::io::copy(&mut file, &mut stdout)
                        .and_then(|count| stdout.flush().map(|()| count));
                    match result {
                        Ok(count) => offset += count,
                        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {
                            return Ok(());
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if !follow {
                        eprintln!("No web server log yet");
                    }
                }
                Err(error) => return Err(error.into()),
            }
            if !follow {
                return Ok(());
            }
            sleep(Duration::from_millis(100));
        }
    }

    pub(super) fn manage(options: WebOptions) -> Result<()> {
        let directory = get_to_tui_dir()?.join("web");
        fs::create_dir_all(&directory)?;
        let log = directory.join("server.log");
        let record_path = directory.join("process.json");
        if options.log || matches!(options.command, Some(WebCommand::Logs { .. })) {
            eprintln!("Log: {}", log.display());
            let follow = matches!(options.command, Some(WebCommand::Logs { follow: true }));
            return print_logs(&log, follow);
        }
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(directory.join("lock"))?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(std::io::Error::last_os_error())
                .context("Another web management command is running; retry shortly");
        }
        let mut record = active(&record_path)?;
        let mut stop_path = record_path.clone();
        if record.is_none() {
            record = legacy_record()?;
            if record.is_some() {
                stop_path = to_tui::utils::paths::get_pid_file_path()?;
            }
        }
        match options.command {
            Some(WebCommand::Status) => {
                if let Some(record) = record {
                    describe(&record, &log);
                } else {
                    println!("Not running. Last log: {}", log.display());
                }
                return Ok(());
            }
            Some(WebCommand::Stop) => return stop(record, &stop_path),
            _ => {}
        }
        if options.restart || matches!(options.command, Some(WebCommand::Restart)) {
            stop(record.take(), &stop_path)?;
        }
        if let Some(record) = record {
            describe(&record, &log);
            bail!("Already started; use web restart to change options");
        }
        if options.command.is_none() && !options.detach {
            drop(lock);
            return crate::run_server_foreground(options.port, options.open, options.verbose);
        }
        start(&options, &directory, &record_path, &log)
    }

    fn start(options: &WebOptions, directory: &Path, record_path: &Path, log: &Path) -> Result<()> {
        let ready: PathBuf = directory.join(format!("ready-{}", uuid::Uuid::new_v4()));
        let output = tempfile::NamedTempFile::new_in(directory)?.persist(log)?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .args(["web", "--port", &options.port.to_string()])
            .env("TOTUI_WEB_READY_FILE", &ready)
            .stdin(Stdio::null())
            .stdout(output.try_clone()?)
            .stderr(output);
        if options.open {
            command.arg("--open");
        }
        if options.verbose {
            command.arg("--verbose");
        }
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command.spawn().context("Failed to launch web server")?;
        let result = (|| -> Result<()> {
            let stamp = identity(child.id())?.context("Web server exited during startup")?;
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                if let Some(status) = child.try_wait()? {
                    bail!(
                        "Web server exited ({status}):\n{}",
                        fs::read_to_string(log)?
                    );
                }
                if ready.exists() {
                    let mut url = String::new();
                    File::open(&ready)?.read_to_string(&mut url)?;
                    if !url.is_empty() {
                        let record = Record {
                            pid: child.id(),
                            identity: stamp,
                            url,
                        };
                        let mut file = tempfile::NamedTempFile::new_in(directory)?;
                        file.write_all(&serde_json::to_vec(&record)?)?;
                        file.persist(record_path)?;
                        describe(&record, log);
                        return Ok(());
                    }
                }
                if Instant::now() >= deadline {
                    bail!("Web startup timed out. Check {}", log.display());
                }
                sleep(Duration::from_millis(50));
            }
        })();
        if result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if ready.exists() {
            fs::remove_file(ready)?;
        }
        result
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_stale_record_does_not_target_reused_pid() {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("process.json");
            let record = Record {
                pid: std::process::id(),
                identity: "different process".into(),
                url: "http://localhost/".into(),
            };
            fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
            assert!(active(&path).unwrap().is_none());
            assert!(!path.exists());
        }
    }
}
