use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use todo_cli::mcp::TodoMcpServer;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .init();

    info!("Starting todo-mcp server");

    let server = TodoMcpServer::new();
    let service = server.serve(stdio()).await?;

    info!("Server ready, waiting for requests...");

    service.waiting().await?;

    info!("Server shutting down");
    Ok(())
}
