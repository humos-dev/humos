//! humos-mcp entrypoint.
//!
//! Three modes:
//!   humos-mcp          (default: stdio MCP server)
//!   humos-mcp doctor   (diagnose daemon connectivity)
//!   humos-mcp version  (print version)

use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use humos_mcp::ipc_client::IpcClient;
use humos_mcp::mcp::server::run_stdio;
use humos_mcp::mcp::tools::ToolDispatcher;

#[derive(Parser)]
#[command(name = "humos-mcp", about = "humOS MCP stdio server")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Serve MCP on stdio (default).
    Serve,
    /// Diagnose daemon connectivity and print setup hints.
    Doctor,
    /// Print version and exit.
    Version,
}

fn main() -> Result<()> {
    // MCP clients pipe JSON on stdout so log must not contaminate stdout.
    // Route everything to stderr.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .target(env_logger::Target::Stderr)
        .init();

    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(),
        Command::Doctor => humos_mcp::doctor::run(),
        Command::Version => {
            println!("humos-mcp {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn serve() -> Result<()> {
    let client = Arc::new(IpcClient::new(IpcClient::default_socket()));
    let dispatcher = Arc::new(ToolDispatcher::new(client));
    run_stdio(dispatcher)
}
