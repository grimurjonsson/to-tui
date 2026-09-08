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
    /// Install and manage the Linux systemd server
    Server {
        #[command(subcommand)]
        command: ServerCommand,
    },
    /// Run or manage the local web workspace
    Web(WebOptions),
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
        /// Require OAuth authentication and isolate each user's data
        #[arg(long, global = true)]
        auth: bool,
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
    /// Claude Code hook entry points. Not intended to be run by hand.
    Hook {
        #[command(subcommand)]
        command: HookCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ServerCommand {
    /// Install the server and enable startup on boot (requires root)
    #[command(alias = "wizard")]
    Install(ServerInstallOptions),
    /// Run the server in the foreground
    Run {
        #[arg(long, default_value_t = DEFAULT_API_PORT, value_parser = clap::value_parser!(u16).range(1024..))]
        port: u16,
        /// Require OAuth authentication and isolate each user's data
        #[arg(long)]
        auth: bool,
    },
    /// Show the systemd service status
    Status,
    /// Show server logs from the journal
    Logs {
        #[arg(short, long)]
        follow: bool,
    },
    /// Start the installed service
    Start,
    /// Stop the installed service
    Stop,
    /// Restart the installed service
    Restart,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ServerInstallOptions {
    /// Require OAuth authentication and isolate each user's data
    #[arg(long)]
    pub auth: bool,
    /// Local port for the existing reverse proxy
    #[arg(long, value_parser = clap::value_parser!(u16).range(1024..))]
    pub port: Option<u16>,
    /// IANA timezone for daily lists, for example Europe/Oslo (default: UTC)
    #[arg(long)]
    pub timezone: Option<String>,
    /// Use supplied options and defaults without interactive prompts
    #[arg(short, long)]
    pub yes: bool,
    /// Print the installation and service unit without changing the machine
    #[arg(long)]
    pub dry_run: bool,
    /// Replace a previously installed to-tui service and binary; preserve data
    #[arg(long)]
    pub replace: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum HookCommand {
    /// Handle a Claude Code `Stop` event: read the hook payload on stdin and,
    /// when the session's totui tree has drifted, print a reminder for the model.
    /// Prints nothing and exits 0 when there is nothing to say.
    Stop {
        /// Project holding the tracked tree
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Handle a Claude Code `SessionStart` event: when the session is resuming or
    /// coming back from a compaction, restate which tree it is driving.
    SessionStart {
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Handle a Claude Code `SessionEnd` event: release this session's claim on
    /// its tree so it can be picked up again.
    SessionEnd,
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
    /// Re-parent a todo, carrying its children with it
    Move {
        /// UUID of the todo to move
        id: String,
        /// UUID of the new parent. Omit to move it to the top level.
        #[arg(long)]
        parent: Option<String>,
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

#[derive(clap::Args, Debug, Clone)]
pub struct WebOptions {
    /// Require OAuth authentication and isolate each user's data
    #[arg(long, global = true)]
    pub auth: bool,
    #[command(subcommand)]
    pub command: Option<WebCommand>,
    #[arg(short, long, global = true, default_value_t = DEFAULT_API_PORT)]
    pub port: u16,
    /// Open the workspace in your browser
    #[arg(long, global = true)]
    pub open: bool,
    /// Log mutation payloads, including task text
    #[arg(long, global = true)]
    pub verbose: bool,
    /// Run in the background
    #[arg(long, global = true, conflicts_with = "log")]
    pub detach: bool,
    /// Stop the managed web server before starting
    #[arg(long, global = true, conflicts_with = "log")]
    pub restart: bool,
    /// Print the latest detached server log and exit
    #[arg(long, global = true, conflicts_with_all = ["open", "verbose"])]
    pub log: bool,
}

#[derive(Subcommand, Debug, Clone, Copy)]
pub enum WebCommand {
    /// Start the workspace in the background
    Start,
    /// Stop the managed web server
    Stop,
    /// Restart the workspace in the background
    Restart,
    /// Show the managed process, URL, and log location
    Status,
    /// Print the latest detached server log
    #[command(alias = "log")]
    Logs {
        /// Keep streaming new log output until interrupted
        #[arg(short, long)]
        follow: bool,
    },
}

#[cfg(test)]
mod web_cli_tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_arguments_are_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn test_web_start_accepts_options_before_and_after_subcommand() {
        for args in [
            vec!["to-tui", "web", "--port", "3000", "start", "--verbose"],
            vec!["to-tui", "web", "start", "--port", "3000", "--verbose"],
        ] {
            let Some(Commands::Web(options)) = Cli::try_parse_from(args).unwrap().command else {
                panic!("Expected web command");
            };
            assert_eq!(options.port, 3000);
            assert!(options.verbose);
            assert!(matches!(options.command, Some(WebCommand::Start)));
        }
    }

    #[test]
    fn test_web_logs_follow() {
        for flag in ["--follow", "-f"] {
            let cli = Cli::try_parse_from(["totui", "web", "logs", flag]).unwrap();
            assert!(matches!(
                cli.command,
                Some(Commands::Web(WebOptions {
                    command: Some(WebCommand::Logs { follow: true }),
                    ..
                }))
            ));
        }
    }

    #[test]
    fn test_web_log_rejects_start_flags() {
        assert!(Cli::try_parse_from(["totui", "web", "--log", "--detach"]).is_err());
    }
}
