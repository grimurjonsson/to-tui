mod api;
mod app;
mod cli;
mod config;
mod storage;
mod todo;
mod ui;
mod utils;

use anyhow::Result;
use chrono::Local;
use clap::Parser;
use cli::{Cli, Commands};
use config::Config;
use storage::{check_and_prompt_rollover, save_todo_list};
use ui::theme::Theme;

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
        Some(Commands::Serve { port }) => {
            handle_serve(port)?;
        }
        None => {
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

#[tokio::main]
async fn handle_serve(port: u16) -> Result<()> {
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
    // Check for rollover first
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
