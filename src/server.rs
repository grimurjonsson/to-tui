use crate::cli::ServerCommand;

use anyhow::Result;

pub fn run(command: ServerCommand) -> Result<()> {
    #[cfg(target_os = "linux")]
    return linux::run(command);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = command;
        anyhow::bail!("Server installation and management require Linux with systemd")
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use crate::cli::{DEFAULT_API_PORT, ServerCommand, ServerInstallOptions};

    use anyhow::{Context, Result, bail, ensure};
    use dialoguer::Input;

    use std::fs;
    use std::io::{IsTerminal, Write};
    use std::net::{Ipv4Addr, TcpListener};
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{SocketAddr, UnixDatagram};
    use std::path::Path;
    use std::process::Command;
    use std::time::{Duration, Instant};

    const SERVICE: &str = "totui.service";
    const UNIT_PATH: &str = "/etc/systemd/system/totui.service";
    const BINARY_PATH: &str = "/usr/local/lib/totui/totui";
    const MARKER: &str = "Description=to-tui personal todo server";

    pub(super) fn run(command: ServerCommand) -> Result<()> {
        match command {
            ServerCommand::Install(options) => install(options),
            ServerCommand::Run { port, auth } => {
                start_watchdog(port)?;
                crate::run_server_foreground(port, false, false, auth)
            }
            ServerCommand::Status => {
                let status = Command::new("systemctl")
                    .args(["status", "--no-pager", SERVICE])
                    .status()
                    .context("Could not run systemctl")?;
                ensure!(status.success(), "Service is not running; see status above");
                Ok(())
            }
            ServerCommand::Logs { follow } => {
                let mut command = Command::new("journalctl");
                command.args(["--unit", SERVICE, "--lines", "100"]);
                command.arg(if follow { "--follow" } else { "--no-pager" });
                checked(&mut command)
            }
            ServerCommand::Start => systemctl(&["start", SERVICE]),
            ServerCommand::Stop => systemctl(&["stop", SERVICE]),
            ServerCommand::Restart => systemctl(&["restart", SERVICE]),
        }
    }

    fn checked(command: &mut Command) -> Result<()> {
        let status = command
            .status()
            .with_context(|| format!("Could not execute {command:?}"))?;
        ensure!(status.success(), "Command failed: {command:?} ({status})");
        Ok(())
    }

    fn systemctl(args: &[&str]) -> Result<()> {
        checked(Command::new("systemctl").args(args))
    }

    fn validate_timezone(timezone: &str) -> Result<()> {
        ensure!(
            !timezone.is_empty()
                && !timezone.starts_with('/')
                && timezone
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
                && timezone
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"/_+-".contains(&c)),
            "Use an IANA timezone such as Europe/Oslo or UTC"
        );
        ensure!(
            Path::new("/usr/share/zoneinfo").join(timezone).is_file(),
            "Timezone {timezone:?} is unavailable; install tzdata or select another timezone"
        );
        Ok(())
    }

    fn unit(port: u16, timezone: &str, auth: bool) -> Result<String> {
        ensure!(
            port >= 1024,
            "Choose an unprivileged port between 1024 and 65535"
        );
        validate_timezone(timezone)?;
        let auth_flag = if auth { " --auth" } else { "" };
        Ok(format!(
            "[Unit]\n{MARKER}\nAfter=network.target\nStartLimitIntervalSec=120\nStartLimitBurst=10\n\n\
             [Service]\nType=notify\nNotifyAccess=main\nWatchdogSec=60\nTimeoutStartSec=60\nDynamicUser=yes\nStateDirectory=totui\nStateDirectoryMode=0700\n\
             WorkingDirectory=/var/lib/totui\nEnvironment=TOTUI_DATA_DIR=/var/lib/totui\n\
             Environment=TOTUI_BIND=127.0.0.1\nEnvironment=TZ={timezone}\n\
             ExecStart={BINARY_PATH} server run --port {port}{auth_flag}\n\
             Restart=on-failure\nRestartSec=5\nTimeoutStopSec=30\nUMask=0077\n\
             NoNewPrivileges=yes\nPrivateTmp=yes\nPrivateDevices=yes\nProtectSystem=strict\n\
             ProtectHome=yes\nProtectKernelTunables=yes\nProtectKernelModules=yes\n\
             ProtectControlGroups=yes\nRestrictSUIDSGID=yes\nRestrictAddressFamilies=AF_UNIX AF_INET AF_INET6\n\
             CapabilityBoundingSet=\nStandardOutput=journal\nStandardError=journal\n\n\
             [Install]\nWantedBy=multi-user.target\n"
        ))
    }

    fn start_watchdog(port: u16) -> Result<()> {
        let Some(socket) = std::env::var_os("NOTIFY_SOCKET") else {
            return Ok(());
        };
        use std::os::unix::ffi::OsStrExt;
        let bytes = socket.as_bytes();
        let address = if let Some(name) = bytes.strip_prefix(b"@") {
            SocketAddr::from_abstract_name(name)?
        } else {
            SocketAddr::from_pathname(&socket)?
        };
        let socket = UnixDatagram::unbound()?;
        socket.set_write_timeout(Some(Duration::from_secs(2)))?;
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()?;
        std::thread::Builder::new()
            .name("systemd-watchdog".into())
            .spawn(move || {
                loop {
                    if client
                        .get(format!("http://127.0.0.1:{port}/api/ready"))
                        .send()
                        .is_ok_and(|response| response.status().is_success())
                        && let Err(error) = socket.send_to_addr(b"READY=1\nWATCHDOG=1", &address)
                    {
                        tracing::error!(%error, "Could not notify systemd");
                    }
                    std::thread::sleep(Duration::from_secs(10));
                }
            })?;
        Ok(())
    }

    fn atomic_write(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
        let parent = path.parent().context("Installation path has no parent")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("Could not create {}", parent.display()))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(contents)?;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(mode))?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(path)
            .with_context(|| format!("Could not install {}", path.display()))?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    }

    fn install(options: ServerInstallOptions) -> Result<()> {
        let existing = match fs::read_to_string(UNIT_PATH) {
            Ok(contents) => Some(contents),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error).context("Could not read the existing service unit"),
        };
        if let Some(existing) = &existing {
            ensure!(
                existing.lines().any(|line| line == MARKER),
                "{UNIT_PATH} belongs to another service; refusing to overwrite it"
            );
            ensure!(
                options.replace || options.dry_run,
                "Server is already installed. Use --replace to update it; settings and data are preserved."
            );
        }
        let (previous_port, previous_timezone) = existing_settings(existing.as_deref());
        let mut port = options.port.or(previous_port).unwrap_or(DEFAULT_API_PORT);
        let mut timezone = options
            .timezone
            .clone()
            .or(previous_timezone)
            .unwrap_or_else(|| "UTC".into());
        if !options.yes && !options.dry_run {
            ensure!(
                std::io::stdin().is_terminal(),
                "Use --yes for noninteractive installation, or --dry-run to preview"
            );
            if options.port.is_none() {
                port = Input::new()
                    .with_prompt("Local reverse-proxy port")
                    .default(port)
                    .interact_text()?;
            }
            if options.timezone.is_none() {
                timezone = Input::new()
                    .with_prompt("Timezone for daily lists")
                    .default(timezone)
                    .interact_text()?;
            }
        }
        let auth = options.auth
            || existing.as_ref().is_some_and(|unit| {
                unit.lines().any(|line| {
                    line.starts_with("ExecStart=")
                        && line.split_whitespace().any(|arg| arg == "--auth")
                })
            });
        let contents = unit(port, &timezone, auth)?;
        println!(
            "Binary: {BINARY_PATH}\nService: {UNIT_PATH}\nPersistent data: /var/lib/totui\nProxy upstream: http://127.0.0.1:{port}\nTimezone: {timezone}"
        );
        println!(
            "The existing reverse proxy must enforce authentication for the entire site, including /api/*."
        );
        if options.dry_run {
            println!("\n{contents}");
            println!(
                "Installation copies the binary, reloads systemd, enables totui.service on boot, and starts it."
            );
            return Ok(());
        }
        ensure!(
            unsafe { libc::geteuid() } == 0,
            "Installation requires root to write /etc/systemd/system and /usr/local/lib. Run sudo totui server install with the same options."
        );
        ensure!(
            Path::new("/run/systemd/system").is_dir(),
            "This machine is not running systemd"
        );
        ensure!(
            !Path::new(BINARY_PATH).exists() || options.replace,
            "{BINARY_PATH} already exists; use --replace to update it"
        );
        let fragment = Command::new("systemctl")
            .args(["show", "--property=FragmentPath", "--value", SERVICE])
            .output()
            .context("Could not inspect existing systemd service")?;
        ensure!(
            fragment.status.success(),
            "Could not inspect existing systemd service"
        );
        let fragment = String::from_utf8(fragment.stdout)?;
        ensure!(
            fragment.trim().is_empty() || fragment.trim() == UNIT_PATH,
            "A service named {SERVICE} is already defined at {}; refusing to override it",
            fragment.trim()
        );
        let running = Command::new("systemctl")
            .args(["is-active", "--quiet", SERVICE])
            .status()?
            .success();
        if !running || previous_port != Some(port) {
            TcpListener::bind((Ipv4Addr::LOCALHOST, port)).with_context(|| {
                format!("Port {port} is unavailable; select another with --port")
            })?;
        }
        let executable = std::env::current_exe().context("Could not locate the running binary")?;
        let bytes = fs::read(&executable).context("Could not read the running binary")?;
        atomic_write(Path::new(BINARY_PATH), &bytes, 0o755)?;
        atomic_write(Path::new(UNIT_PATH), contents.as_bytes(), 0o644)?;
        systemctl(&["daemon-reload"])?;
        systemctl(&["enable", SERVICE])?;
        systemctl(&[if running { "restart" } else { "start" }, SERVICE])?;
        wait_until_ready(port)?;
        println!(
            "Server is ready and enabled on boot. Use totui server status or totui server logs --follow."
        );
        Ok(())
    }

    fn existing_settings(unit: Option<&str>) -> (Option<u16>, Option<String>) {
        let Some(unit) = unit else {
            return (None, None);
        };
        let port_prefix = format!("ExecStart={BINARY_PATH} server run --port ");
        let port = unit.lines().find_map(|line| {
            line.strip_prefix(&port_prefix).and_then(|value| {
                value
                    .split_whitespace()
                    .next()
                    .and_then(|port| port.parse().ok())
            })
        });
        let timezone = unit
            .lines()
            .find_map(|line| line.strip_prefix("Environment=TZ=").map(str::to_owned));
        (port, timezone)
    }

    fn wait_until_ready(port: u16) -> Result<()> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .no_proxy()
            .build()?;
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if client
                .get(format!("http://127.0.0.1:{port}/api/ready"))
                .send()
                .is_ok_and(|response| response.status().is_success())
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        bail!(
            "Service was installed but did not become ready. Inspect totui server logs and totui server status; data has been preserved."
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::cli::{Cli, Commands};
        use clap::Parser;

        #[test]
        fn test_server_install_rejects_invalid_ports() {
            for port in ["0", "80", "65536"] {
                assert!(
                    Cli::try_parse_from(["totui", "server", "install", "--port", port]).is_err()
                );
            }
        }

        #[test]
        fn test_server_wizard_alias() {
            let cli = Cli::try_parse_from(["totui", "server", "wizard", "--dry-run"]).unwrap();
            assert!(
                matches!(cli.command, Some(Commands::Server { command: ServerCommand::Install(options) }) if options.dry_run)
            );
        }

        #[test]
        fn test_existing_settings_preserve_nondefault_installation() {
            let content = unit(49372, "Etc/GMT+3", false).unwrap();
            assert_eq!(
                existing_settings(Some(&content)),
                (Some(49372), Some("Etc/GMT+3".into()))
            );
        }

        #[test]
        fn test_timezone_rejects_unit_injection_and_traversal() {
            for timezone in [
                "UTC\nExecStart=/bin/false",
                "../UTC",
                "/etc/passwd",
                "UTC%",
                "UTC\"",
                "",
            ] {
                assert!(validate_timezone(timezone).is_err());
            }
        }

        #[test]
        fn test_unit_uses_persistent_storage_and_unprivileged_loopback_service() {
            let content = unit(49372, "UTC", false).unwrap();
            assert!(content.contains("DynamicUser=yes\nStateDirectory=totui"));
            assert!(content.contains("Environment=TOTUI_BIND=127.0.0.1"));
            assert!(content.contains("server run --port 49372"));
            assert!(content.contains("WantedBy=multi-user.target"));
        }

        #[test]
        fn test_atomic_write_replaces_binary_without_changing_open_file() {
            use std::io::Read;
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("totui");
            atomic_write(&path, b"old", 0o755).unwrap();
            let mut old = fs::File::open(&path).unwrap();
            atomic_write(&path, b"new", 0o755).unwrap();
            let mut old_contents = String::new();
            old.read_to_string(&mut old_contents).unwrap();
            assert_eq!(old_contents, "old");
            assert_eq!(fs::read(&path).unwrap(), b"new");
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
    }
}
