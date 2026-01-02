mod api;
mod app;
mod cli;
mod config;
mod storage;
mod todo;
mod ui;
mod utils;

use anyhow::{anyhow, Result};
use chrono::Local;
use clap::Parser;
use cli::{Cli, Commands, ServeCommand, DEFAULT_API_PORT};
use config::Config;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::Duration;
use storage::{check_and_prompt_rollover, save_todo_list};
use ui::theme::Theme;
use utils::paths::get_pid_file_path;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Some(Commands::Add { task }) => {
            handle_add(task)?;
        }
        Some(Commands::Show) => {
            handle_show()?;
        }
        Some(Commands::Serve { command, port }) => {
            handle_serve_command(command, port)?;
        }
        None => {
            ensure_server_running(DEFAULT_API_PORT)?;

            let list = check_and_prompt_rollover()?.unwrap_or_else(|| {
                let today = Local::now().date_naive();
                todo::TodoList::new(today, utils::paths::get_daily_file_path(today).unwrap())
            });

            let theme = Theme::from_config(&config);
            let state = app::AppState::new(list, theme);

            ui::run_tui(state)?;
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
        println!("Server is already running on port {}", port);
        return Ok(());
    }

    start_server_background(port)?;
    println!("Server started on port {}", port);
    Ok(())
}

fn handle_serve_stop() -> Result<()> {
    let pid = read_pid_file()?;
    
    if let Some(pid) = pid {
        kill_process(pid)?;
        remove_pid_file()?;
        println!("Server stopped (PID: {})", pid);
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
            println!("Server is running on port {} (PID: {})", port, pid);
        }
        (Some(pid), false) => {
            println!("Server PID file exists ({}) but server is not responding on port {}", pid, port);
            println!("Consider running 'todo serve stop' to clean up");
        }
        (None, true) => {
            println!("Server is running on port {} but no PID file found", port);
        }
        (None, false) => {
            println!("Server is not running");
        }
    }
    
    Ok(())
}

fn is_server_running(port: u16) -> bool {
    let addr = format!("127.0.0.1:{}", port);
    match TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(500)) {
        Ok(mut stream) => {
            let request = format!(
                "GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
                port
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
        return Err(anyhow!("Failed to start server - not responding on port {}", port));
    }
    
    Ok(())
}

fn ensure_server_running(port: u16) -> Result<()> {
    if !is_server_running(port) {
        println!("Starting API server on port {}...", port);
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
    
    if let Some(parent) = pid_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
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
    let addr = format!("0.0.0.0:{}", port);

    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn handle_add(task: String) -> Result<()> {
    let mut list = check_and_prompt_rollover()?.unwrap_or_else(|| {
        let today = Local::now().date_naive();
        todo::TodoList::new(today, utils::paths::get_daily_file_path(today).unwrap())
    });

    list.add_item(task);
    save_todo_list(&list)?;

    println!("✓ Todo added successfully!");

    Ok(())
}

fn handle_show() -> Result<()> {
    let list = check_and_prompt_rollover()?.unwrap_or_else(|| {
        let today = Local::now().date_naive();
        todo::TodoList::new(today, utils::paths::get_daily_file_path(today).unwrap())
    });

    if list.is_empty() {
        println!("No todos for today!");
        return Ok(());
    }

    println!("\n📋 Todo List - {}\n", list.date.format("%B %d, %Y"));

    for (idx, item) in list.items.iter().enumerate() {
        let indent = "  ".repeat(item.indent_level);
        println!("{}{}. {} {}", indent, idx + 1, item.state, item.content);
    }

    println!();

    Ok(())
}
