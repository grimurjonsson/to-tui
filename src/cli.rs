use clap::{Parser, Subcommand};

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
    Show,
    Serve {
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
}
