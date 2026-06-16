//! clap argument structs for the subcommands.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use url::Url;

#[derive(Subcommand, Debug)]
pub enum ProjectsCmd {
    /// List registered projects
    List,
    /// Register a project
    Add {
        name: String,
        #[arg(long)]
        base_url: Url,
        /// On-disk openapi.json fallback used when the server is down
        #[arg(long)]
        spec_file: Option<PathBuf>,
        /// Default header sent with every request, as "Key: Value" (repeatable)
        #[arg(short = 'H', long = "header")]
        headers: Vec<String>,
    },
    /// Unregister a project (tokens and cache are also removed)
    Remove { name: String },
}

#[derive(Subcommand, Debug)]
pub enum SpecCmd {
    /// Re-fetch openapi.json and refresh the cache
    Refresh { project: String },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Validate projects.toml
    Check,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    pub project: String,
    /// operation_id or "METHOD /path"
    pub endpoint: String,
    /// JSON body: inline string, @file.json, or '-' for stdin
    #[arg(long)]
    pub body: Option<String>,
    /// Path parameter, as name=value (repeatable)
    #[arg(short = 'p', long = "path-param", value_name = "NAME=VALUE")]
    pub path_params: Vec<String>,
    /// Query parameter, as name=value (repeatable)
    #[arg(short = 'q', long = "query", value_name = "NAME=VALUE")]
    pub query: Vec<String>,
    /// Extra header, as "Key: Value" (repeatable)
    #[arg(short = 'H', long = "header", value_name = "KEY: VALUE")]
    pub headers: Vec<String>,
    /// Skip authentication for this request
    #[arg(long)]
    pub no_auth: bool,
    /// Exit 0 even when the response is an HTTP error
    #[arg(long)]
    pub allow_error: bool,
}
