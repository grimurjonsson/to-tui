mod api;
mod app;
mod cli;
mod ui;

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
use cli::{Cli, Commands, DEFAULT_API_PORT, PluginCommand, ServeCommand};
use config::Config;
use plugin::{PluginActionRegistry, PluginLoader, PluginManager};
use plugin::config::{generate_config_template, PluginConfigLoader};
use utils::paths::{get_logs_dir, get_plugin_config_dir, get_plugin_config_path};
use keybindings::KeybindingCache;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::panic;
use std::process::{Command, Stdio};
use std::time::Duration;
use project::{Project, ProjectRegistry, DEFAULT_PROJECT_NAME};
use storage::file::{file_exists_for_project, load_todo_list_for_project};
use storage::file::save_todo_list_for_project;
use storage::{ensure_installation_ready, find_rollover_candidates_for_project, UiCache};
use ui::theme::Theme;
use utils::paths::{get_crash_log_path, get_daily_file_path_for_project, get_pid_file_path};

/// Load today's todo list for a specific project without prompting for rollover.
/// Creates an empty list if no existing todos are found.
fn load_today_list_for_project(project_name: &str) -> Result<todo::TodoList> {
    let today = Local::now().date_naive();
    if file_exists_for_project(project_name, today)? {
        load_todo_list_for_project(project_name, today)
    } else {
        Ok(todo::TodoList::new(
            today,
            get_daily_file_path_for_project(project_name, today)?,
        ))
    }
}

/// Get the current project from config or default
fn get_current_project(config: &Config) -> Result<Project> {
    let mut registry = ProjectRegistry::load()?;
    registry.ensure_default_project()?;

    // Try to use last_used_project from config
    if let Some(ref last_project_name) = config.last_used_project
        && let Some(project) = registry.get_by_name(last_project_name)
    {
        return Ok(project.clone());
    }

    // Fall back to default project
    Ok(registry
        .get_by_name(DEFAULT_PROJECT_NAME)
        .expect("Default project must exist after ensure_default_project")
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
            crash_report.push_str(&format!("\nBacktrace:\n{}\n", std::backtrace::Backtrace::force_capture()));
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
/// Logs are written to ~/.local/share/to-tui/logs/totui.log
/// Use `tail -f ~/.local/share/to-tui/logs/totui.log` to follow logs.
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

    // Set up file appender (rolling daily)
    let file_appender = tracing_appender::rolling::daily(&logs_dir, "totui.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Initialize subscriber with file output
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false) // No ANSI colors in log files
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

    // Ensure installation is properly set up (handles v1 -> v2 migration)
    ensure_installation_ready()?;

    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Some(Commands::Add { task }) => {
            handle_add(task)?;
        }
        Some(Commands::Show { date }) => {
            handle_show(date)?;
        }
        Some(Commands::ImportArchive) => {
            handle_import_archive()?;
        }
        Some(Commands::Serve { command, port }) => {
            handle_serve_command(command, port)?;
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
        None => {
            // Initialize file logging for TUI mode
            // Guard must be kept alive for the duration of the app
            let _log_guard = init_file_logging();

            tracing::info!("totui starting");

            ensure_server_running(DEFAULT_API_PORT)?;

            // Determine which project to load
            let current_project = get_current_project(&config)?;
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
            let (mut plugin_errors, config_errors) = plugin_loader.load_all_with_config(&plugin_manager);

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

                    let warnings = registry.register_plugin(
                        &plugin_info.manifest,
                        &overrides,
                        &keybindings,
                    );

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
            );

            // Check for rollover candidates and show modal on startup if found
            if let Ok(Some((source_date, items))) = find_rollover_candidates_for_project(&state.current_project.name) {
                state.open_rollover_modal(source_date, items);
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

fn handle_serve_command(command: Option<ServeCommand>, port: u16) -> Result<()> {
    match command.unwrap_or(ServeCommand::Start { daemon: false }) {
        ServeCommand::Start { daemon } => {
            if daemon {
                run_server_foreground(port)
            } else {
                handle_serve_start(port)
            }
        }
        ServeCommand::Stop => handle_serve_stop(),
        ServeCommand::Restart => handle_serve_restart(port),
        ServeCommand::Status => handle_serve_status(port),
    }
}

fn handle_serve_start(port: u16) -> Result<()> {
    if is_server_running(port) {
        println!("Server is already running on port {port}");
        return Ok(());
    }

    start_server_background(port)?;
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

fn handle_serve_restart(port: u16) -> Result<()> {
    let _ = handle_serve_stop();
    std::thread::sleep(Duration::from_millis(500));
    handle_serve_start(port)
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

fn start_server_background(port: u16) -> Result<()> {
    let current_exe = env::current_exe()?;

    let child = Command::new(&current_exe)
        .args(["serve", "start", "--port", &port.to_string(), "--daemon"])
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

fn ensure_server_running(port: u16) -> Result<()> {
    if !is_server_running(port) {
        println!("Starting API server on port {port}...");
        start_server_background(port)?;
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
        && !parent.exists() {
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

#[tokio::main]
async fn run_server_foreground(port: u16) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .init();

    let app = api::create_router();
    let addr = format!("0.0.0.0:{port}");

    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn handle_add(task: String) -> Result<()> {
    let mut list = load_today_list_for_project(DEFAULT_PROJECT_NAME)?;

    list.add_item(task);
    save_todo_list_for_project(&list, DEFAULT_PROJECT_NAME)?;

    println!("✓ Todo added successfully!");

    Ok(())
}

fn handle_show(date: Option<String>) -> Result<()> {
    let (items, display_date, is_archived): (Vec<todo::TodoItem>, chrono::NaiveDate, bool) =
        if let Some(date_str) = date {
            let parsed_date = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|_| anyhow!("Invalid date format. Use YYYY-MM-DD"))?;

            let today = Local::now().date_naive();
            if parsed_date == today {
                let list = load_today_list_for_project(DEFAULT_PROJECT_NAME)?;
                (list.items, today, false)
            } else {
                let items = storage::load_archived_todos_for_date_and_project(parsed_date, DEFAULT_PROJECT_NAME)?;
                (items, parsed_date, true)
            }
        } else {
            let list = load_today_list_for_project(DEFAULT_PROJECT_NAME)?;
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

    let label = if is_archived {
        "📦 Archived"
    } else {
        "📋 Todo List"
    };
    println!("\n{} - {}\n", label, display_date.format("%B %d, %Y"));

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

    Ok(selections
        .into_iter()
        .map(|i| items[i].clone())
        .collect())
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
        PluginCommand::Install { source, version, force } => {
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
                    if field.field_type == totui_plugin_interface::FfiConfigType::Select && !field.options.is_empty() {
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
