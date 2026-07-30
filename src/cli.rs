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

        /// Filter by project name
        #[arg(short, long)]
        project: Option<String>,
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
    /// Read and write todos as JSON (for scripts, hooks and automation)
    Todo {
        #[command(subcommand)]
        command: TodoCommand,
    },
}

/// Scriptable todo access. Every subcommand prints JSON on stdout and a plain
/// message on stderr when it fails, so callers can pipe stdout straight into `jq`.
#[derive(Subcommand, Debug, Clone)]
pub enum TodoCommand {
    /// Create a todo. Supply --json or the individual flags.
    Create {
        /// Full spec as a JSON object, or `-` to read it from stdin.
        /// Keys: content, description, state, due_date, parent_id, priority.
        #[arg(long, conflicts_with = "content")]
        json: Option<String>,

        /// Todo text (required unless --json is given)
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// ' ' pending, '*' in progress, 'x' done, '?' question, '!' important, '-' cancelled
        #[arg(long)]
        state: Option<String>,
        /// Due date, YYYY-MM-DD
        #[arg(long)]
        due_date: Option<String>,
        /// UUID of the parent todo, to nest this one under it
        #[arg(long)]
        parent_id: Option<String>,
        /// p0, p1 or p2
        #[arg(long)]
        priority: Option<String>,

        #[arg(short, long)]
        project: Option<String>,
        /// Day the todo lives on, YYYY-MM-DD. Defaults to today.
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Update an existing todo. Omitted fields are left unchanged.
    Update {
        /// UUID of the todo
        id: String,

        /// Patch as a JSON object, or `-` to read it from stdin.
        /// Keys: content, description, state, due_date, priority.
        #[arg(long)]
        json: Option<String>,

        #[arg(long)]
        content: Option<String>,
        /// Pass an empty string to clear the description
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        due_date: Option<String>,
        #[arg(long)]
        priority: Option<String>,

        #[arg(short, long)]
        project: Option<String>,
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Print one todo as JSON
    Get {
        /// UUID of the todo
        id: String,
        #[arg(short, long)]
        project: Option<String>,
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Print every todo for a project/date as a JSON array
    List {
        #[arg(short, long)]
        project: Option<String>,
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Delete a todo and all of its children
    Delete {
        /// UUID of the todo
        id: String,
        #[arg(short, long)]
        project: Option<String>,
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Print the available project names as a JSON array
    Projects,
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
