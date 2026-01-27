use clap::{Parser, Subcommand};

/// Default port for the API server
pub const DEFAULT_API_PORT: u16 = 48372;

#[derive(Parser, Debug)]
#[command(name = "totui")]
#[command(version)]
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
    /// Generate todos from external sources using plugins
    Generate {
        /// Generator name (e.g., 'jira')
        generator: Option<String>,

        /// Input for the generator (e.g., ticket ID)
        input: Option<String>,

        /// List available generators
        #[arg(short, long)]
        list: bool,

        /// Auto-confirm adding all generated todos
        #[arg(short, long)]
        yes: bool,
    },
    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum PluginCommand {
    /// List installed plugins
    List,
    /// Install a plugin from local directory or GitHub
    Install {
        /// Plugin source: local path or owner/repo/plugin-name
        source: String,
        /// Version to install (remote only, default: latest)
        #[arg(long)]
        version: Option<String>,
        /// Overwrite existing installation
        #[arg(long)]
        force: bool,
    },
    /// Enable a plugin
    Enable {
        /// Plugin name
        name: String,
    },
    /// Disable a plugin
    Disable {
        /// Plugin name
        name: String,
    },
    /// Show detailed plugin status
    Status {
        /// Plugin name
        name: String,
    },
    /// Validate a plugin's configuration
    Validate {
        /// Plugin name
        name: String,
    },
    /// Show or initialize plugin config
    Config {
        /// Plugin name
        name: String,
        /// Generate template config from schema
        #[arg(long)]
        init: bool,
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
