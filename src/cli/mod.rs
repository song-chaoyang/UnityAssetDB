mod index_cmd;
mod mcp_cmd;
mod query_cmd;
mod serve_cmd;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub use index_cmd::run_index_action;
pub use mcp_cmd::run_mcp_server;
pub use query_cmd::{run_glob, run_grep, run_ls, run_read, run_refs};
pub use serve_cmd::run_serve;

#[derive(Parser)]
#[command(name = "unityassetdb")]
#[command(about = "Every Unity asset reference in one queryable database — built in Rust")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Index management
    Index {
        #[command(subcommand)]
        action: IndexAction,
    },
    /// Query references for a VFS path
    Refs {
        /// VFS path to query (e.g. Assets/Materials/Enemy.mat)
        path: String,
        /// Project root path
        #[arg(short, long)]
        project: Option<PathBuf>,
        /// Direction: in (who references this) or out (what this references)
        #[arg(short, long, default_value = "in")]
        direction: String,
        /// Filter: File, Component, GameObject, or ALL
        #[arg(short, long, default_value = "ALL")]
        filter: String,
    },
    /// List VFS entries under a path
    Ls {
        path: String,
        #[arg(short, long)]
        project: Option<PathBuf>,
        #[arg(short, long, default_value = "1")]
        depth: usize,
    },
    /// Find VFS entries matching a glob pattern
    Glob {
        pattern: String,
        #[arg(short, long)]
        project: Option<PathBuf>,
        #[arg(short, long, default_value = "ALL")]
        entry_type: String,
    },
    /// Search content within VFS entries
    Grep {
        pattern: String,
        #[arg(short, long)]
        project: Option<PathBuf>,
        /// Only search within this VFS path prefix
        #[arg(long)]
        path: Option<String>,
    },
    /// Read content of a VFS entry
    Read {
        path: String,
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Start web server with interactive graph UI
    Serve {
        /// Project root path
        project: PathBuf,
        #[arg(short, long, default_value = "8089")]
        port: u16,
    },
    /// Start MCP server (JSON-RPC over stdio) for MCP protocol integration
    Mcp,
}

#[derive(Subcommand)]
pub enum IndexAction {
    /// Build a fresh index for a Unity project
    Build {
        /// Unity project root path
        project: PathBuf,
        /// Output database path (default: <project>/.unityassetdb/index.db)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Include Packages directory
        #[arg(long, default_value = "true")]
        packages: bool,
    },
    /// Incrementally update the index
    Sync { project: PathBuf },
    /// Show index status and metadata
    Status { project: PathBuf },
}
