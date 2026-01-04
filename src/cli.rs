use clap::{Parser, Subcommand};

/// Default port for the API server
pub const DEFAULT_API_PORT: u16 = 48372;

#[derive(Parser, Debug)]
#[command(name = "todo")]
#[command(about = "A terminal-based todo list manager with daily rolling lists", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Add {
        task: String,
    },
    Show {
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Import old markdown files into the archive
    ImportArchive,
    /// Manage the API server
    Serve {
        #[command(subcommand)]
        command: Option<ServeCommand>,

        /// Port to run the server on
        #[arg(short, long, global = true, default_value_t = DEFAULT_API_PORT)]
        port: u16,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ServeCommand {
    /// Start the API server (default if no subcommand given)
    Start {
        #[arg(long, hide = true)]
        daemon: bool,
    },
    /// Stop the running API server
    Stop,
    /// Restart the API server
    Restart,
    /// Check if the API server is running
    Status,
}
