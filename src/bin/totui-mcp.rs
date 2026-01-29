use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use std::env;
use to_tui::mcp::TodoMcpServer;
use tracing::{debug, error, info};
use tracing_subscriber::{EnvFilter, fmt};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<()> {
    // Check for version flag before anything else
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("totui-mcp {}", VERSION);
        return Ok(());
    }

    // Check for --debug / -v flags to increase verbosity
    let verbose = args.iter().any(|a| a == "--debug" || a == "-v" || a == "--verbose");
    let default_filter = if verbose { "debug" } else { "info" };
    
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter)),
        )
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .init();

    info!(version = VERSION, "Starting totui-mcp server");
    debug!("Debug logging enabled");
    debug!(args = ?args, "Command line arguments");

    debug!("Initializing TodoMcpServer...");
    let server = TodoMcpServer::new();
    debug!("TodoMcpServer created successfully");

    debug!("Setting up stdio transport...");
    info!("Connecting via stdio transport (stdin/stdout)...");
    
    let service = match server.serve(stdio()).await {
        Ok(s) => {
            info!("MCP service created successfully");
            debug!("stdio transport connected");
            s
        }
        Err(e) => {
            error!(error = %e, "Failed to create MCP service");
            anyhow::bail!("Failed to create MCP service: {}", e);
        }
    };

    info!("Server ready, waiting for requests on stdio...");
    debug!("Entering main service loop");

    service.waiting().await.map_err(|e| anyhow::anyhow!("Service error: {}", e))?;

    info!("Server shutting down gracefully");
    Ok(())
}
