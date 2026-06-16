//! clap definitions and headless command handlers.

mod args;
mod handlers;
pub mod output;
mod parse;

use std::io::IsTerminal;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::AppServices;

pub use args::{ConfigCmd, ProjectsCmd, RunArgs, SpecCmd};

#[derive(Parser, Debug)]
#[command(
    name = "hit",
    version,
    about = "Browse and hit your projects' APIs — interactively or from scripts/agents",
    long_about = "hitpoint: a terminal API tester for FastAPI backends.\n\
                  Run with no arguments for the interactive TUI. Every subcommand supports\n\
                  --json for machine-readable output (automatic when stdout is not a TTY)."
)]
pub struct Cli {
    /// Path to projects.toml (defaults to ~/.config/hitpoint/projects.toml)
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Emit a JSON envelope {ok, data, error} on stdout
    #[arg(long, global = true)]
    pub json: bool,

    /// Increase log verbosity (-v info, -vv debug)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Bypass the spec cache and re-fetch openapi.json
    #[arg(long, global = true)]
    pub no_cache: bool,

    /// Request timeout in seconds (overrides settings.timeout_secs)
    #[arg(long, global = true, value_name = "SECS")]
    pub timeout: Option<u64>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Manage registered projects
    Projects {
        #[command(subcommand)]
        cmd: ProjectsCmd,
    },
    /// List a project's OpenAPI tags
    Tags { project: String },
    /// List endpoints, optionally filtered by tag or search string
    Endpoints {
        project: String,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        search: Option<String>,
    },
    /// Print a fill-in-the-blanks request template for an endpoint
    Template {
        project: String,
        /// operation_id or "METHOD /path"
        endpoint: String,
    },
    /// Execute a request against an endpoint
    Run(RunArgs),
    /// Authenticate now and cache the token
    Login { project: String },
    /// Clear the cached token
    Logout { project: String },
    /// Spec cache operations
    Spec {
        #[command(subcommand)]
        cmd: SpecCmd,
    },
    /// Config file operations
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// Run as an MCP server on stdio (for AI agents)
    Mcp,
    /// Open the interactive TUI (same as running with no arguments)
    Tui { project: Option<String> },
    /// Generate shell completions
    Completions { shell: clap_complete::Shell },
}

/// Whether output should be the JSON envelope.
pub fn json_mode(cli: &Cli) -> bool {
    cli.json || !std::io::stdout().is_terminal()
}

/// Run a headless subcommand; returns the process exit code.
pub async fn run(cli: Cli, services: AppServices) -> i32 {
    let json = json_mode(&cli);
    let Some(command) = cli.command else {
        unreachable!("no-subcommand launches the TUI from main");
    };
    let result = handlers::dispatch(command, &cli.config, cli.no_cache, services).await;
    output::print_result(json, result)
}
