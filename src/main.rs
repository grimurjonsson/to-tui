use to_tui::api;
mod app;
mod cli;
mod remote_cli;
mod server;
mod ui;
mod web_process;

use to_tui::clipboard;
use to_tui::config;
use to_tui::keybindings;
use to_tui::plugin;
use to_tui::project;
use to_tui::storage;
use to_tui::todo;
use to_tui::utils;

use anyhow::{Result, anyhow};
use chrono::Local;
use clap::Parser;
use cli::{Cli, Commands, DEFAULT_API_PORT, HookCommand, PluginCommand, ServeCommand, TodoCommand};
use config::Config;
use keybindings::KeybindingCache;
use plugin::config::{PluginConfigLoader, generate_config_template};
use plugin::{PluginActionRegistry, PluginLoader, PluginManager};
use project::{DEFAULT_PROJECT_NAME, Project, ProjectRegistry};
use std::env;
use std::fs;
use std::future::IntoFuture;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::panic;
use std::process::{Command, Stdio};
use std::time::Duration;
use storage::file::save_todo_list_for_project;
use storage::file::{file_exists_for_project, load_todo_list_for_project};
use storage::{UiCache, ensure_installation_ready, find_rollover_candidates_for_project};
use ui::theme::Theme;
use utils::paths::{get_crash_log_path, get_daily_file_path_for_project, get_pid_file_path};
use utils::paths::{get_logs_dir, get_plugin_config_dir, get_plugin_config_path};

/// Load today's todo list for a specific project without prompting for rollover.
/// Creates an empty list if no existing todos are found.
fn load_today_list_for_project(project_name: &str) -> Result<todo::TodoList> {
    let today = Local::now().date_naive();
    if to_tui::remote::active()
        .and_then(|client| client.cached())
        .is_some()
        || file_exists_for_project(project_name, today)?
    {
        load_todo_list_for_project(project_name, today)
    } else {
        Ok(todo::TodoList::new(
            today,
            get_daily_file_path_for_project(project_name, today)?,
        ))
    }
}

/// Get the current project: the one mapped to the current folder if any,
/// else the last used project, else the default. Records the folder binding
/// so subsequent launches from this folder are stable.
fn get_current_project(config: &mut Config) -> Result<Project> {
    let mut registry = ProjectRegistry::load()?;
    registry.ensure_default_project()?;

    let folder_key = project::current_folder_key();
    let name = project::resolve_project_name(
        folder_key.as_deref(),
        &config.folder_projects,
        config.last_used_project.as_deref(),
        |n| registry.get_by_name(n).is_some(),
    );

    if let Some(key) = folder_key
        && config.folder_projects.get(&key) != Some(&name)
    {
        config.folder_projects.insert(key, name.clone());
        let _ = config.save();
    }

    Ok(registry
        .get_by_name(&name)
        .expect("Resolved project must exist in registry")
        .clone())
}

/// Install a panic hook that writes crash information to a log file
fn install_crash_handler() {
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |panic_info| {
        // Try to write to crash log
        if let Ok(crash_log_path) = get_crash_log_path() {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
            let mut crash_report = format!("=== CRASH at {} ===\n", timestamp);

            // Add panic message
            if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
                crash_report.push_str(&format!("Message: {}\n", message));
            } else if let Some(message) = panic_info.payload().downcast_ref::<String>() {
                crash_report.push_str(&format!("Message: {}\n", message));
            }

            // Add location if available
            if let Some(location) = panic_info.location() {
                crash_report.push_str(&format!(
                    "Location: {}:{}:{}\n",
                    location.file(),
                    location.line(),
                    location.column()
                ));
            }

            // Add backtrace
            crash_report.push_str(&format!(
                "\nBacktrace:\n{}\n",
                std::backtrace::Backtrace::force_capture()
            ));
            crash_report.push('\n');

            // Append to crash log
            if let Ok(mut file) = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&crash_log_path)
            {
                let _ = file.write_all(crash_report.as_bytes());
                eprintln!("\nCrash logged to: {}", crash_log_path.display());
            }
        }

        // Call the default hook (prints to stderr)
        default_hook(panic_info);
    }));
}

/// Initialize file-based logging for the TUI mode.
///
/// Logs are written to ~/.to-tui/logs/totui.log
/// Use `tail -f ~/.to-tui/logs/totui.log` to follow logs with colors.
///
/// Log level can be controlled with RUST_LOG env var (default: info).
fn init_file_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let logs_dir = match get_logs_dir() {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    // Create logs directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&logs_dir) {
        eprintln!("Warning: Could not create logs directory: {}", e);
        return None;
    }

    // Roll previous log file if it's from a different day
    let log_file_path = logs_dir.join("totui.log");
    if log_file_path.exists()
        && let Ok(metadata) = fs::metadata(&log_file_path)
        && let Ok(modified) = metadata.modified()
    {
        let modified_date = chrono::DateTime::<Local>::from(modified)
            .format("%Y-%m-%d")
            .to_string();
        let today = Local::now().format("%Y-%m-%d").to_string();
        if modified_date != today {
            let rolled_name = logs_dir.join(format!("totui.log.{}", modified_date));
            let _ = fs::rename(&log_file_path, rolled_name);
        }
    }

    // Write to a stable totui.log file (never-rolling appender)
    let file_appender = tracing_appender::rolling::never(&logs_dir, "totui.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Initialize subscriber with file output
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(true) // Enable ANSI colors in log files for better readability
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .init();

    Some(guard)
}

fn main() -> Result<()> {
    // Install crash handler first thing
    install_crash_handler();

    let cli = Cli::parse();
    if let Some(Commands::Remote { command }) = &cli.command {
        return remote_cli::run(command.clone());
    }
    if cli.remote.is_some()
        && !matches!(
            cli.command,
            None | Some(Commands::Add { .. } | Commands::Show { .. } | Commands::Todo { .. })
        )
    {
        anyhow::bail!("--remote is supported for the TUI, add, show and todo commands");
    }
    if let Some(Commands::Server { command }) = &cli.command {
        return server::run(command.clone());
    }
    if let Some(Commands::Web(options)) = &cli.command {
        return web_process::run(options.clone());
    }

    let mut config = Config::load()?;
    let selected_remote = if cli.local {
        None
    } else {
        cli.remote.or_else(|| config.default_remote.clone())
    };
    if let Some(name) = selected_remote.as_deref() {
        if !matches!(
            cli.command,
            None | Some(Commands::Add { .. } | Commands::Show { .. } | Commands::Todo { .. })
        ) {
            anyhow::bail!("This command requires local mode. Use --local explicitly");
        }
        let profile = config
            .remotes
            .get(name)
            .ok_or_else(|| anyhow!("Unknown remote '{name}'"))?
            .clone();
        let use_cache = cli.command.is_none();
        let mut client = to_tui::remote::Client::configured(name, profile.clone())?;
        let root = utils::paths::get_to_tui_dir()?;
        let warm_cache = profile.user_id.is_some()
            && to_tui::remote::cache::Cache::exists(&client.workspace_path(root.clone()));
        if !use_cache || !warm_cache {
            let user = client.user()?;
            let mut verified = profile;
            verified.user_id = Some(user.id);
            client = to_tui::remote::Client::configured(name, verified)?;
            client.check()?;
        }
        if use_cache {
            let path = client.workspace_path(root);
            client = client.with_cache(&path)?;
        }
        to_tui::remote::activate(client)?;
        fs::create_dir_all(utils::paths::get_to_tui_dir()?)?;
        let projects = ProjectRegistry::load()?;
        config = Config::load_for_remote(&config, |name| projects.get_by_name(name).is_some())?;
    } else {
        ensure_installation_ready()?;
    }

    match cli.command {
        Some(Commands::Remote { command }) => remote_cli::run(command)?,
        Some(Commands::Server { command }) => server::run(command)?,
        Some(Commands::Add { task }) => {
            handle_add(task)?;
        }
        Some(Commands::Show { date, project }) => {
            handle_show(date, project)?;
        }
        Some(Commands::ImportArchive) => {
            handle_import_archive()?;
        }
        Some(Commands::Web(options)) => {
            web_process::run(options)?;
        }
        Some(Commands::Serve {
            command,
            port,
            auth,
        }) => {
            handle_serve_command(command, port, auth)?;
        }
        Some(Commands::Generate {
            generator,
            input,
            list,
            yes,
        }) => {
            handle_generate(generator, input, list, yes)?;
        }
        Some(Commands::Plugin { command }) => {
            handle_plugin_command(command)?;
        }
        Some(Commands::Todo { command }) => {
            handle_todo_command(command, selected_remote.as_deref())?;
        }
        Some(Commands::Hook { command }) => {
            handle_hook_command(command);
        }
        None => {
            // Initialize file logging for TUI mode
            // Guard must be kept alive for the duration of the app
            let _log_guard = init_file_logging();

            tracing::info!("totui starting");

            if to_tui::remote::active().is_none() {
                ensure_server_running(DEFAULT_API_PORT)?;
            }

            // Determine which project to load
            let current_project = get_current_project(&mut config)?;
            let list = load_today_list_for_project(&current_project.name)?;

            // Load UI cache for restoring cursor position
            let ui_cache = UiCache::load().ok();

            let theme = Theme::from_config(&config);
            let keybindings = KeybindingCache::from_config(&config.keybindings);

            // Discover plugins and load config
            let mut plugin_manager = PluginManager::discover()?;
            plugin_manager.apply_config(&config.plugins);

            // Load dynamic plugins with config validation
            let mut plugin_loader = PluginLoader::new();
            let (mut plugin_errors, config_errors) =
                plugin_loader.load_all_with_config(&plugin_manager);

            // Log load errors
            if !plugin_errors.is_empty() {
                tracing::warn!("{} plugin(s) failed to load", plugin_errors.len());
                for error in &plugin_errors {
                    tracing::debug!("Plugin error: {} - {}", error.plugin_name, error.message);
                }
            }

            // Log config errors separately with "config" context
            if !config_errors.is_empty() {
                tracing::warn!("{} plugin(s) failed config validation", config_errors.len());
                for error in &config_errors {
                    tracing::warn!(
                        plugin = %error.plugin_name,
                        config = true,
                        "Config error: {}",
                        error.message
                    );
                }
            }

            // Convert config errors to PluginLoadError for unified display in popup
            let config_as_load_errors: Vec<plugin::PluginLoadError> = config_errors
                .into_iter()
                .map(|ce| plugin::PluginLoadError {
                    plugin_name: ce.plugin_name,
                    error_kind: plugin::PluginErrorKind::Other(format!("Config: {}", ce.message)),
                    message: ce.message,
                })
                .collect();
            plugin_errors.extend(config_as_load_errors);

            // Build plugin action registry from loaded plugins
            let plugin_action_registry = {
                let mut registry = PluginActionRegistry::new();

                // Get plugin keybinding overrides from config
                let plugin_overrides = &config.keybindings.plugins;

                // Register actions from plugin manager's discovered plugins
                for plugin_info in plugin_manager.list() {
                    if !plugin_info.enabled || !plugin_info.available {
                        continue;
                    }

                    let overrides = plugin_overrides
                        .get(&plugin_info.manifest.name)
                        .cloned()
                        .unwrap_or_default();

                    let warnings =
                        registry.register_plugin(&plugin_info.manifest, &overrides, &keybindings);

                    for warning in warnings {
                        tracing::warn!("{}", warning);
                    }
                }

                registry
            };

            let mut state = app::AppState::new(
                list,
                theme,
                keybindings,
                config.timeoutlen,
                ui_cache,
                config.skipped_version.clone(),
                current_project,
                plugin_loader,
                plugin_errors,
                plugin_action_registry,
                config.auto_rollover,
            );

            // Apply the rollover preference for any incomplete items left over
            // from a previous day. Honors auto_rollover (AutoYes rolls silently,
            // Ask prompts, AutoNo does nothing) — same logic as the midnight tick.
            match find_rollover_candidates_for_project(&state.current_project.name) {
                Ok(candidates) => state.apply_rollover_preference(candidates),
                Err(e) => tracing::error!("Startup rollover candidate lookup failed: {e}"),
            }

            // Fire OnLoad event to subscribed plugins
            state.fire_on_load_event();

            // Log loaded plugins count (uses plugin_loader field)
            let loaded_count = state.loaded_plugin_count();
            if loaded_count > 0 {
                tracing::info!("{} dynamic plugin(s) loaded", loaded_count);
            }

            let state = ui::run_tui(state)?;

            tracing::info!("totui exiting gracefully");

            // Print release URL if user requested it
            if let Some(url) = state.pending_release_url {
                println!("\nNew version available:");
                println!("{}", url);
            }
        }
    }

    Ok(())
}

fn handle_serve_command(command: Option<ServeCommand>, port: u16, auth: bool) -> Result<()> {
    match command.unwrap_or(ServeCommand::Start { daemon: false }) {
        ServeCommand::Start { daemon } => {
            if daemon {
                run_server_foreground(port, false, false, auth)
            } else {
                handle_serve_start(port, auth)
            }
        }
        ServeCommand::Stop => handle_serve_stop(),
        ServeCommand::Restart => handle_serve_restart(port, auth),
        ServeCommand::Status => handle_serve_status(port),
    }
}

fn handle_serve_start(port: u16, auth: bool) -> Result<()> {
    if is_server_running(port) {
        anyhow::ensure!(
            !auth,
            "Server is already running; use serve restart --auth to enable authentication"
        );
        println!("Server is already running on port {port}");
        return Ok(());
    }

    start_server_background(port, auth)?;
    println!("Server started on port {port}");
    Ok(())
}

fn handle_serve_stop() -> Result<()> {
    let pid = read_pid_file()?;

    if let Some(pid) = pid {
        kill_process(pid)?;
        remove_pid_file()?;
        println!("Server stopped (PID: {pid})");
    } else {
        println!("Server is not running (no PID file found)");
    }

    Ok(())
}

fn handle_serve_restart(port: u16, auth: bool) -> Result<()> {
    let auth = auth || web_process::authentication_enabled()?;
    let _ = handle_serve_stop();
    std::thread::sleep(Duration::from_millis(500));
    handle_serve_start(port, auth)
}

fn handle_serve_status(port: u16) -> Result<()> {
    let pid = read_pid_file()?;
    let running = is_server_running(port);

    match (pid, running) {
        (Some(pid), true) => {
            println!("Server is running on port {port} (PID: {pid})");
        }
        (Some(pid), false) => {
            println!("Server PID file exists ({pid}) but server is not responding on port {port}");
            println!("Consider running 'todo serve stop' to clean up");
        }
        (None, true) => {
            println!("Server is running on port {port} but no PID file found");
        }
        (None, false) => {
            println!("Server is not running");
        }
    }

    Ok(())
}

fn is_server_running(port: u16) -> bool {
    let addr = format!("127.0.0.1:{port}");
    match TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(500)) {
        Ok(mut stream) => {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
            let request = format!(
                "GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
            );
            if stream.write_all(request.as_bytes()).is_ok() {
                let mut response = String::new();
                let _ = stream.read_to_string(&mut response);
                response.contains("200") || response.contains("ok")
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

fn start_server_background(port: u16, auth: bool) -> Result<()> {
    let current_exe = env::current_exe()?;

    let mut command = Command::new(&current_exe);
    command.args(["serve", "start", "--port", &port.to_string(), "--daemon"]);
    if auth {
        command.arg("--auth");
    }
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    write_pid_file(child.id())?;

    std::thread::sleep(Duration::from_millis(500));

    if !is_server_running(port) {
        return Err(anyhow!(
            "Failed to start server - not responding on port {port}"
        ));
    }

    Ok(())
}

#[cfg(unix)]
fn ensure_server_running(port: u16) -> Result<()> {
    if matches!(web_process::status(port)?, web_process::WebStatus::Stopped) {
        let output = Command::new(env::current_exe()?)
            .args(["web", "start", "--port", &port.to_string()])
            .output()?;
        if !output.status.success() {
            tracing::warn!(
                "Web startup failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_server_running(port: u16) -> Result<()> {
    if !is_server_running(port) {
        start_server_background(port, false)?;
    }
    Ok(())
}

fn read_pid_file() -> Result<Option<u32>> {
    let pid_path = get_pid_file_path()?;

    if !pid_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&pid_path)?;
    let pid: u32 = content.trim().parse()?;
    Ok(Some(pid))
}

fn write_pid_file(pid: u32) -> Result<()> {
    let pid_path = get_pid_file_path()?;

    if let Some(parent) = pid_path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }

    fs::write(&pid_path, pid.to_string())?;
    Ok(())
}

fn remove_pid_file() -> Result<()> {
    let pid_path = get_pid_file_path()?;
    if pid_path.exists() {
        fs::remove_file(&pid_path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn kill_process(pid: u32) -> Result<()> {
    use std::process::Command;
    Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()?;
    Ok(())
}

#[cfg(windows)]
fn kill_process(pid: u32) -> Result<()> {
    use std::process::Command;
    Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output()?;
    Ok(())
}

fn workspace_url(addr: std::net::SocketAddr, project: &str) -> Result<reqwest::Url> {
    let host = if addr.ip().is_unspecified() {
        if addr.is_ipv6() {
            "[::1]".to_owned()
        } else {
            "127.0.0.1".to_owned()
        }
    } else if addr.is_ipv6() {
        format!("[{}]", addr.ip())
    } else {
        addr.ip().to_string()
    };
    let mut url = reqwest::Url::parse(&format!("http://{host}:{}/", addr.port()))?;
    url.query_pairs_mut().append_pair("project", project);
    Ok(url)
}

#[tokio::main]
async fn run_server_foreground(
    port: u16,
    open_browser: bool,
    verbose: bool,
    auth: bool,
) -> Result<()> {
    fs::create_dir_all(utils::paths::get_to_tui_dir()?)?;
    if !auth {
        ensure_installation_ready()?;
    }
    let mut filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,tower_http=debug".into());
    if verbose {
        filter = filter.add_directive("to_tui::api=debug".parse()?);
    }
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let (app, project_name) = if auth {
        let auth_url =
            std::env::var("TOTUI_AUTH_URL").unwrap_or_else(|_| api::auth::DEFAULT_AUTH_URL.into());
        (
            api::create_authenticated_router(&auth_url)?,
            DEFAULT_PROJECT_NAME.to_owned(),
        )
    } else {
        let project = get_current_project(&mut Config::load()?)?;
        (
            api::create_router()?.layer(axum::Extension(api::web::StartupProject(
                project.name.clone(),
            ))),
            project.name,
        )
    };
    let host = std::env::var("TOTUI_BIND").unwrap_or_else(|_| "127.0.0.1".into());
    let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
    let addr = listener.local_addr()?;
    let url = workspace_url(addr, &project_name)?;
    tracing::info!("Workspace available at {url}");
    if open_browser {
        open::that(url.as_str())?;
    }
    if let Some(path) = std::env::var_os("TOTUI_WEB_READY_FILE") {
        fs::write(path, url.as_str())?;
    }
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = stopped.await;
        })
        .into_future();
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => result?,
        _ = server_shutdown() => {
            let _ = shutdown.send(());
            if let Ok(result) = tokio::time::timeout(Duration::from_secs(2), &mut server).await {
                result?;
            }
        }
    }

    Ok(())
}

async fn server_shutdown() {
    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = terminate.recv() => {},
                    _ = tokio::signal::ctrl_c() => {},
                }
            }
            Err(error) => {
                tracing::error!(%error, "Could not register SIGTERM handler");
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

fn handle_add(task: String) -> Result<()> {
    let mut list = load_today_list_for_project(DEFAULT_PROJECT_NAME)?;

    list.add_item(task);
    save_todo_list_for_project(&list, DEFAULT_PROJECT_NAME)?;

    println!("✓ Todo added successfully!");

    Ok(())
}

fn handle_show(date: Option<String>, project: Option<String>) -> Result<()> {
    let project_name = project.as_deref().unwrap_or(DEFAULT_PROJECT_NAME);

    // Validate project exists
    let mut registry = project::ProjectRegistry::load()?;
    registry.ensure_default_project()?;
    if registry.get_by_name(project_name).is_none() {
        anyhow::bail!("Project '{}' not found", project_name);
    }

    let (items, display_date, is_archived): (Vec<todo::TodoItem>, chrono::NaiveDate, bool) =
        if let Some(date_str) = date {
            let parsed_date = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|_| anyhow!("Invalid date format. Use YYYY-MM-DD"))?;

            let today = Local::now().date_naive();
            if parsed_date == today {
                let list = load_today_list_for_project(project_name)?;
                (list.items, today, false)
            } else {
                let items =
                    storage::load_archived_todos_for_date_and_project(parsed_date, project_name)?;
                (items, parsed_date, true)
            }
        } else {
            let list = load_today_list_for_project(project_name)?;
            let date = list.date;
            (list.items, date, false)
        };

    if items.is_empty() {
        if is_archived {
            println!(
                "No archived todos for {}!",
                display_date.format("%B %d, %Y")
            );
        } else {
            println!("No todos for today!");
        }
        return Ok(());
    }

    let project_label = if project_name != DEFAULT_PROJECT_NAME {
        format!(" [{}]", project_name)
    } else {
        String::new()
    };
    let label = if is_archived {
        "📦 Archived"
    } else {
        "📋 Todo List"
    };
    println!(
        "\n{}{} - {}\n",
        label,
        project_label,
        display_date.format("%B %d, %Y")
    );

    for (idx, item) in items.iter().enumerate() {
        let indent = "  ".repeat(item.indent_level);
        println!("{}{}. {} {}", indent, idx + 1, item.state, item.content);
    }

    println!();

    Ok(())
}

fn handle_generate(
    generator: Option<String>,
    input: Option<String>,
    list: bool,
    yes: bool,
) -> Result<()> {
    use plugin::{PluginLoader, PluginManager};

    // Discover and load plugins
    let plugin_manager = PluginManager::discover()?;
    let mut plugin_loader = PluginLoader::new();
    let _load_errors = plugin_loader.load_all(&plugin_manager);

    if list {
        println!("\nAvailable generators (external plugins):\n");
        let plugins: Vec<_> = plugin_loader.loaded_plugins().collect();
        if plugins.is_empty() {
            println!("  No plugins installed.");
            println!("  Install plugins with: totui plugin install <plugin>");
        } else {
            for plugin in plugins {
                let status = if plugin.session_disabled {
                    "\x1b[31m[disabled]\x1b[0m"
                } else {
                    "\x1b[32m[available]\x1b[0m"
                };
                println!(
                    "  {} v{} - {} {}",
                    plugin.name, plugin.version, plugin.description, status
                );
            }
        }
        println!();
        return Ok(());
    }

    let generator_name = generator.ok_or_else(|| {
        anyhow!(
            "Generator name required. Use --list to see available generators.\n\
             Usage: todo generate <generator> <input>"
        )
    })?;

    let input_value = input.ok_or_else(|| {
        anyhow!(
            "Input required for generator '{generator_name}'.\n\
             Usage: todo generate {generator_name} <input>"
        )
    })?;

    // Check if plugin is loaded
    if plugin_loader.get(&generator_name).is_none() {
        return Err(anyhow!(
            "Generator '{generator_name}' not found. Use --list to see available generators.\n\
             Install plugins with: totui plugin install <plugin>"
        ));
    }

    println!("Fetching data from {generator_name}...");
    let items = plugin_loader
        .call_generate(&generator_name, &input_value)
        .map_err(|e| anyhow!("{}", e.message))?;

    println!("\nGenerated {} todo(s):\n", items.len());
    for (i, item) in items.iter().enumerate() {
        let indent = "  ".repeat(item.indent_level);
        println!("  {}{}. [ ] {}", indent, i + 1, item.content);
    }
    println!();

    let items_count = items.len();

    if yes {
        add_items_to_today(items)?;
        println!("\x1b[32m✓ Added {items_count} todo(s) to today's list!\x1b[0m");
        return Ok(());
    }

    use dialoguer::Select;

    let choices = vec![
        "Yes - Add all to today's list",
        "No - Cancel",
        "Select - Choose which to add",
    ];

    let selection = Select::new()
        .with_prompt("Add these todos to today's list?")
        .items(&choices)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            add_items_to_today(items)?;
            println!("\n\x1b[32m✓ Added {items_count} todo(s) to today's list!\x1b[0m");
        }
        1 => {
            println!("\nCancelled.");
        }
        2 => {
            let selected = select_items_interactive(&items)?;
            if selected.is_empty() {
                println!("\nNo items selected.");
            } else {
                let count = selected.len();
                add_items_to_today(selected)?;
                println!("\n\x1b[32m✓ Added {count} todo(s) to today's list!\x1b[0m");
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn add_items_to_today(items: Vec<todo::TodoItem>) -> Result<()> {
    let mut list = load_today_list_for_project(DEFAULT_PROJECT_NAME)?;

    for item in items {
        list.items.push(item);
    }

    save_todo_list_for_project(&list, DEFAULT_PROJECT_NAME)?;
    Ok(())
}

fn select_items_interactive(items: &[todo::TodoItem]) -> Result<Vec<todo::TodoItem>> {
    use dialoguer::MultiSelect;

    let display_items: Vec<String> = items
        .iter()
        .map(|item| {
            let indent = "  ".repeat(item.indent_level);
            format!("{}[ ] {}", indent, item.content)
        })
        .collect();

    let selections = MultiSelect::new()
        .with_prompt("Select items to add (space to toggle, enter to confirm)")
        .items(&display_items)
        .interact()?;

    Ok(selections.into_iter().map(|i| items[i].clone()).collect())
}

fn handle_import_archive() -> Result<()> {
    use storage::database::{archive_todos_for_date_and_project, init_database};
    use storage::markdown::parse_todo_list;
    use utils::paths::get_dailies_dir_for_project;

    init_database()?;

    let dailies_dir = get_dailies_dir_for_project(DEFAULT_PROJECT_NAME)?;
    if !dailies_dir.exists() {
        println!("No dailies directory found at {dailies_dir:?}");
        return Ok(());
    }

    let today = Local::now().date_naive();
    let mut imported = 0;

    for entry in std::fs::read_dir(&dailies_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "md").unwrap_or(false) {
            let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

            if let Ok(date) = chrono::NaiveDate::parse_from_str(filename, "%Y-%m-%d") {
                if date >= today {
                    println!("Skipping {filename} (today or future)");
                    continue;
                }

                let content = std::fs::read_to_string(&path)?;
                let list = parse_todo_list(&content, date, path.clone())?;

                if list.items.is_empty() {
                    println!("Skipping {filename} (empty)");
                    continue;
                }

                storage::database::save_todo_list_for_project(&list, DEFAULT_PROJECT_NAME)?;
                let count = archive_todos_for_date_and_project(date, DEFAULT_PROJECT_NAME)?;
                println!("Imported {count} items from {filename}");
                imported += count;
            }
        }
    }

    println!("\nTotal: {imported} items imported to archive");
    Ok(())
}

/// Handle a Claude Code hook event.
///
/// This runs at the end of every turn of every session on the machine, so it is
/// built to do nothing loudly: any failure — unparseable payload, missing
/// project, deleted tree — exits quietly with no output. A hook that errors or
/// chatters is worse than no hook at all.
fn handle_hook_command(command: HookCommand) {
    match command {
        HookCommand::Stop { project } => {
            if let Some(output) = hook_stop(project.as_deref())
                && let Ok(json) = serde_json::to_string(&output)
            {
                println!("{json}");
            }
        }
        HookCommand::SessionStart { project } => {
            if let Some(output) = hook_session_start(project.as_deref())
                && let Ok(json) = serde_json::to_string(&output)
            {
                println!("{json}");
            }
        }
        HookCommand::SessionEnd => hook_session_end(),
    }
}

/// Tell a resumed or post-compaction session which tree it is driving.
///
/// A brand-new session is skipped: it has no history to have forgotten, and the
/// skill establishes its own tree.
fn hook_session_start(project: Option<&str>) -> Option<to_tui::hook::HookOutput> {
    use to_tui::hook;
    use to_tui::todo::ops;

    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw).ok()?;
    let payload: hook::SessionStartPayload = serde_json::from_str(&raw).ok()?;
    if payload.session_id.is_empty() || !hook::needs_context_reinjection(&payload.source) {
        return None;
    }

    let state = hook::load_session(&payload.session_id)?;
    let items = ops::list(project, None).ok()?.items;
    let summary = hook::summarize(&items, &state.root_id)?;
    Some(hook::HookOutput::session_start(summary))
}

/// Release this session's claim so its tree can be picked up again, and take the
/// opportunity to sweep claims left behind by sessions that died without a
/// SessionEnd. Prints nothing either way.
fn hook_session_end() {
    use to_tui::hook;

    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() {
        return;
    }
    let Ok(payload) = serde_json::from_str::<hook::SessionEndPayload>(&raw) else {
        return;
    };

    if !payload.session_id.is_empty() && hook::should_release_claim(&payload.reason) {
        let _ = hook::release_session(&payload.session_id);
    }
    hook::sweep_stale_claims(30);
}

fn hook_stop(project: Option<&str>) -> Option<to_tui::hook::HookOutput> {
    use to_tui::hook;
    use to_tui::todo::ops;

    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw).ok()?;
    let payload: hook::StopPayload = serde_json::from_str(&raw).ok()?;
    if payload.session_id.is_empty() {
        return None;
    }

    let items = ops::list(project, None).ok()?.items;
    let existing = hook::load_session(&payload.session_id);

    // No tree claimed yet: adopt one only when the choice is unambiguous.
    let mut state = match existing {
        Some(s) if s.has_claim() => s,
        other => {
            let claimed = hook::roots_claimed_by_others(&payload.session_id);
            let candidates: Vec<String> = hook::candidate_roots(&items)
                .into_iter()
                .filter(|r| !claimed.contains(r))
                .collect();

            match candidates.as_slice() {
                [root] => hook::SessionState {
                    root_id: root.clone(),
                    project: project.unwrap_or_default().to_string(),
                    cwd: payload.cwd.clone(),
                    last_active_leaf: hook::active_leaf_id(&items, root),
                    last_change_turn: payload.turn_number,
                    prompted: other.map(|s| s.prompted).unwrap_or(false),
                },
                // Nothing to claim. Raise the untracked note once, but only when
                // there is agent work sitting there unwatched — a session that
                // simply is not using totui should never hear from this hook.
                [] => {
                    let already_prompted = other.is_some_and(|s| s.prompted);
                    if already_prompted || !hook::has_trackable_work(&items) {
                        return None;
                    }
                    let _ = hook::save_session(
                        &payload.session_id,
                        &hook::SessionState::prompted_only(),
                    );
                    return Some(hook::HookOutput::stop(hook::untracked_note()));
                }
                // Ambiguous — refuse to guess.
                _ => return None,
            }
        }
    };

    // A changed active leaf restarts the staleness clock.
    let current_leaf = hook::active_leaf_id(&items, &state.root_id);
    if current_leaf != state.last_active_leaf {
        state.last_active_leaf = current_leaf;
        state.last_change_turn = payload.turn_number;
    }
    let active_for = payload.turn_number.saturating_sub(state.last_change_turn);

    let _ = hook::save_session(&payload.session_id, &state);

    let finding = hook::evaluate(&items, &state.root_id, Some(active_for))?;
    let root = hook::root_content(&items, &state.root_id)?;
    Some(hook::HookOutput::stop(finding.message(&root)))
}

/// Read a `--json` argument, treating `-` as "read the object from stdin".
/// Stdin matters for hooks, which receive their own JSON and would otherwise
/// have to shell-escape a nested object onto the command line.
fn read_json_arg(arg: &str) -> Result<String> {
    if arg == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        Ok(arg.to_string())
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn handle_todo_command(command: TodoCommand, remote_name: Option<&str>) -> Result<()> {
    use to_tui::todo::ops;

    let registry = ProjectRegistry::load()?;
    let config = Config::load()?;
    let folder = project::current_folder_key();
    let project = match command.project() {
        Some(name) => ops::resolve_project(Some(name)).map_err(|e| anyhow!(e))?,
        None => project::resolve_project_name(
            folder.as_deref(),
            &config.folder_projects,
            config.last_used_project.as_deref(),
            |name| registry.get_by_name(name).is_some(),
        ),
    };
    let client = to_tui::remote::active();
    let backend = if client.is_some() { "remote" } else { "local" };
    let url = client.as_ref().map(|c| c.url());
    if matches!(
        command,
        TodoCommand::Create { .. }
            | TodoCommand::Update { .. }
            | TodoCommand::Move { .. }
            | TodoCommand::Delete { .. }
    ) {
        eprintln!(
            "Destination: {backend} {} | project {project}",
            url.unwrap_or("database")
        );
    }
    let selected_project = project;
    match command {
        TodoCommand::Context { .. } => print_json(&serde_json::json!({
            "backend": backend, "remote": remote_name, "server_url": url,
            "project": selected_project, "folder": folder,
            "directory": std::env::current_dir()?.canonicalize()?,
            "data_directory": utils::paths::get_to_tui_dir()?,
        })),
        TodoCommand::Create {
            json,
            content,
            description,
            state,
            due_date,
            parent_id,
            priority,
            project: _,
            date,
        } => {
            let spec: ops::CreateSpec = match json {
                Some(raw) => serde_json::from_str(&read_json_arg(&raw)?)
                    .map_err(|e| anyhow!("Invalid --json: {e}"))?,
                None => ops::CreateSpec {
                    content: content.ok_or_else(|| anyhow!("Provide --content or --json"))?,
                    description,
                    state,
                    due_date,
                    parent_id,
                    priority,
                },
            };
            let item = ops::create(Some(selected_project.as_str()), date.as_deref(), spec)
                .map_err(|e| anyhow!(e))?;
            print_json(&item)
        }
        TodoCommand::Update {
            id,
            json,
            content,
            description,
            state,
            due_date,
            priority,
            project: _,
            date,
        } => {
            let spec: ops::UpdateSpec = match json {
                Some(raw) => serde_json::from_str(&read_json_arg(&raw)?)
                    .map_err(|e| anyhow!("Invalid --json: {e}"))?,
                None => ops::UpdateSpec {
                    placement: None,
                    expected_revision: None,
                    clear_due_date: false,
                    clear_priority: false,
                    content,
                    description,
                    state,
                    due_date,
                    priority,
                },
            };
            let item = ops::update(Some(selected_project.as_str()), date.as_deref(), &id, spec)
                .map_err(|e| anyhow!(e))?;
            print_json(&item)
        }
        TodoCommand::Move {
            id,
            parent,
            project: _,
            date,
        } => {
            let item = ops::move_item(
                Some(selected_project.as_str()),
                date.as_deref(),
                &id,
                parent.as_deref(),
            )
            .map_err(|e| anyhow!(e))?;
            print_json(&item)
        }
        TodoCommand::Get {
            id,
            project: _,
            date,
        } => {
            let item = ops::get(Some(selected_project.as_str()), date.as_deref(), &id)
                .map_err(|e| anyhow!(e))?;
            print_json(&item)
        }
        TodoCommand::List { project: _, date } => {
            let result = ops::list(Some(selected_project.as_str()), date.as_deref())
                .map_err(|e| anyhow!(e))?;
            // Print the bare array so callers can pipe straight into `jq '.[]'`.
            print_json(&result.items)
        }
        TodoCommand::Delete {
            id,
            project: _,
            date,
        } => {
            let removed = ops::delete(Some(selected_project.as_str()), date.as_deref(), &id)
                .map_err(|e| anyhow!(e))?;
            print_json(&serde_json::json!({ "deleted": removed }))
        }
        TodoCommand::Projects => {
            let registry = ProjectRegistry::load()?;
            let names: Vec<&str> = registry.projects.iter().map(|p| p.name.as_str()).collect();
            print_json(&names)
        }
    }
}

fn handle_plugin_command(command: PluginCommand) -> Result<()> {
    match command {
        PluginCommand::List => {
            let config = Config::load()?;
            let mut manager = PluginManager::discover()?;
            manager.apply_config(&config.plugins);

            let mut plugins: Vec<_> = manager.list().into_iter().collect();
            plugins.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));

            if plugins.is_empty() {
                println!("No plugins installed.");
                println!("\nInstall plugins with: totui plugin install <source>");
                return Ok(());
            }

            // Print header
            println!("{:<20} {:<12} {:<12} SOURCE", "NAME", "VERSION", "STATUS");
            println!("{}", "-".repeat(60));

            for info in plugins {
                let status = if info.error.is_some() {
                    "error"
                } else if !info.available {
                    "incompatible"
                } else if !info.enabled {
                    "disabled"
                } else {
                    "enabled"
                };

                println!(
                    "{:<20} {:<12} {:<12} {}",
                    info.manifest.name, info.manifest.version, status, info.source
                );
            }
            Ok(())
        }
        PluginCommand::Install {
            source,
            version,
            force,
        } => {
            use plugin::installer::{PluginInstaller, PluginSource};

            let mut plugin_source = PluginSource::parse(&source)?;

            // Apply version from CLI arg if provided
            if version.is_some() {
                plugin_source.version = version;
            }

            if plugin_source.is_local() {
                let result = PluginInstaller::install_from_local(
                    plugin_source.local_path.as_ref().unwrap(),
                    force,
                )?;
                println!(
                    "\x1b[32m[OK]\x1b[0m Installed plugin '{}' v{} to {}",
                    result.plugin_name,
                    result.version,
                    result.path.display()
                );
            } else {
                // Resolve latest version if not specified
                if plugin_source.version.is_none() {
                    let latest = PluginInstaller::resolve_latest_version(&plugin_source)?;
                    println!("Resolved latest version: {}", latest);
                    plugin_source.version = Some(latest);
                }

                let result = PluginInstaller::install_from_remote(&plugin_source, force)?;
                println!(
                    "\x1b[32m[OK]\x1b[0m Installed plugin '{}' v{} to {}",
                    result.plugin_name,
                    result.version,
                    result.path.display()
                );
            }
            Ok(())
        }
        PluginCommand::Enable { name } => {
            // Verify plugin exists
            let manager = PluginManager::discover()?;
            if manager.get(&name).is_none() {
                return Err(anyhow!(
                    "Plugin '{}' not found. Run 'totui plugin list' to see installed plugins.",
                    name
                ));
            }

            let mut config = Config::load()?;
            config.plugins.enable(&name);
            config.save()?;
            println!("Plugin '{}' enabled", name);
            Ok(())
        }
        PluginCommand::Disable { name } => {
            // Verify plugin exists
            let manager = PluginManager::discover()?;
            if manager.get(&name).is_none() {
                return Err(anyhow!(
                    "Plugin '{}' not found. Run 'totui plugin list' to see installed plugins.",
                    name
                ));
            }

            let mut config = Config::load()?;
            config.plugins.disable(&name);
            config.save()?;
            println!("Plugin '{}' disabled", name);
            Ok(())
        }
        PluginCommand::Status { name } => {
            let config = Config::load()?;
            let mut manager = PluginManager::discover()?;
            manager.apply_config(&config.plugins);

            match manager.get(&name) {
                Some(info) => {
                    println!("\nPlugin: {}", info.manifest.name);
                    println!("Version: {}", info.manifest.version);
                    println!("Description: {}", info.manifest.description);
                    println!("Path: {:?}", info.path);
                    println!("Enabled: {}", info.enabled);
                    println!("Available: {}", info.available);

                    if let Some(ref reason) = info.availability_reason {
                        println!("Availability: {}", reason);
                    }

                    if let Some(ref author) = info.manifest.author {
                        println!("Author: {}", author);
                    }
                    if let Some(ref license) = info.manifest.license {
                        println!("License: {}", license);
                    }
                    if let Some(ref homepage) = info.manifest.homepage {
                        println!("Homepage: {}", homepage);
                    }
                    if let Some(ref repository) = info.manifest.repository {
                        println!("Repository: {}", repository);
                    }
                    if let Some(ref min_ver) = info.manifest.min_interface_version {
                        println!("Min Interface Version: {}", min_ver);
                    }

                    if let Some(ref err) = info.error {
                        println!("\n\x1b[31mError: {}\x1b[0m", err);
                    }
                    println!();
                }
                None => {
                    println!("Plugin '{}' not found", name);
                    println!("Run 'totui plugin list' to see installed plugins");
                }
            }
            Ok(())
        }
        PluginCommand::Validate { name } => handle_plugin_validate(&name),
        PluginCommand::Config { name, init } => handle_plugin_config(&name, init),
    }
}

fn handle_plugin_validate(name: &str) -> Result<()> {
    // Discover plugins
    let manager = PluginManager::discover()?;

    // Find plugin by name (case-insensitive)
    let plugin_info = manager.get(name).ok_or_else(|| {
        anyhow!(
            "Plugin '{}' not found. Run 'totui plugin list' to see installed plugins.",
            name
        )
    })?;

    // Load the plugin to get schema
    let loader = PluginLoader::new();
    let loaded = loader.load_plugin(&plugin_info.path, plugin_info)?;
    let schema = loaded.plugin.config_schema();

    // Validate config
    match PluginConfigLoader::load_and_validate(&loaded.name, &schema) {
        Ok(config) => {
            println!(
                "\x1b[32m[OK]\x1b[0m Plugin '{}' configuration is valid",
                loaded.name
            );
            println!("  {} field(s) loaded", config.len());
            Ok(())
        }
        Err(e) => {
            eprintln!(
                "\x1b[31m[ERROR]\x1b[0m Plugin '{}' configuration invalid:",
                loaded.name
            );
            eprintln!("  {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_plugin_config(name: &str, init: bool) -> Result<()> {
    // Discover plugins
    let manager = PluginManager::discover()?;

    // Find plugin by name (case-insensitive)
    let plugin_info = manager.get(name).ok_or_else(|| {
        anyhow!(
            "Plugin '{}' not found. Run 'totui plugin list' to see installed plugins.",
            name
        )
    })?;

    let config_path = get_plugin_config_path(&plugin_info.manifest.name)?;
    let config_dir = get_plugin_config_dir(&plugin_info.manifest.name)?;

    if init {
        // Load plugin to get schema
        let loader = PluginLoader::new();
        let loaded = loader.load_plugin(&plugin_info.path, plugin_info)?;
        let schema = loaded.plugin.config_schema();

        // Create config directory
        fs::create_dir_all(&config_dir)?;

        // Generate template
        let template = generate_config_template(&schema);

        // Write to config file
        fs::write(&config_path, template)?;

        println!(
            "\x1b[32m[OK]\x1b[0m Created config template for '{}'",
            loaded.name
        );
        println!("  Path: {}", config_path.display());
        println!("\nEdit this file with your configuration, then run:");
        println!("  totui plugin validate {}", loaded.name);
        Ok(())
    } else {
        // Show config info
        println!("\nPlugin: {}", plugin_info.manifest.name);
        println!("Config path: {}", config_path.display());

        if config_path.exists() {
            println!("Status: \x1b[32mexists\x1b[0m");

            // Load plugin to get schema for summary
            let loader = PluginLoader::new();
            let loaded = loader.load_plugin(&plugin_info.path, plugin_info)?;
            let schema = loaded.plugin.config_schema();

            if !schema.fields.is_empty() {
                println!("\nSchema fields:");
                for field in schema.fields.iter() {
                    let type_name = match field.field_type {
                        totui_plugin_interface::FfiConfigType::String => "string",
                        totui_plugin_interface::FfiConfigType::Integer => "integer",
                        totui_plugin_interface::FfiConfigType::Boolean => "boolean",
                        totui_plugin_interface::FfiConfigType::StringArray => "string[]",
                        totui_plugin_interface::FfiConfigType::Select => "select",
                    };
                    let req = if field.required { "*" } else { "" };
                    println!("  {}{}: {}", field.name, req, type_name);

                    // Show options for Select fields
                    if field.field_type == totui_plugin_interface::FfiConfigType::Select
                        && !field.options.is_empty()
                    {
                        let opts: Vec<_> = field.options.iter().map(|s| s.as_str()).collect();
                        println!("      Options: {}", opts.join(", "));
                    }
                }
                println!("\n  * = required");
            }
        } else {
            println!("Status: \x1b[33mdoes not exist\x1b[0m");
            println!("\nTo create a config template, run:");
            println!("  totui plugin config {} --init", plugin_info.manifest.name);
        }

        println!();
        Ok(())
    }
}

#[cfg(test)]
mod web_startup_tests {
    use super::workspace_url;

    #[test]
    fn test_workspace_url_encodes_selected_project() {
        let url = workspace_url("127.0.0.1:48372".parse().unwrap(), "Notes & ideas #1").unwrap();
        assert_eq!(
            url.query_pairs().collect::<Vec<_>>(),
            vec![("project".into(), "Notes & ideas #1".into())]
        );
        assert!(url.fragment().is_none());
    }

    #[test]
    fn test_workspace_url_uses_reachable_loopback_for_wildcard_bind() {
        for (bind, expected) in [
            ("0.0.0.0:48372", "http://127.0.0.1:48372/?project=default"),
            ("[::]:48372", "http://[::1]:48372/?project=default"),
        ] {
            assert_eq!(
                workspace_url(bind.parse().unwrap(), "default")
                    .unwrap()
                    .as_str(),
                expected
            );
        }
    }
}
