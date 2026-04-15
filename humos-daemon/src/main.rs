//! humos-daemon entrypoint.
//!
//! Three modes:
//!   humos-daemon          (or `humos-daemon run`), run the daemon
//!   humos-daemon doctor   , print a health check report
//!   humos-daemon version  , print version info

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use humos_daemon::config::HumosConfig;
use humos_daemon::index::keyword::KeywordIndexer;
use humos_daemon::ipc::handler::Handler;
use humos_daemon::ipc::IpcServer;
use humos_daemon::scanner::Scanner;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};

#[derive(Parser)]
#[command(name = "humos-daemon", about = "humOS coordination runtime daemon")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon (default).
    Run,
    /// Run diagnostic checks and print a report.
    Doctor,
    /// Print version and exit.
    Version,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Run) {
        Command::Run => run(),
        Command::Doctor => humos_daemon::doctor::run(),
        Command::Version => {
            println!("humos-daemon {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn run() -> Result<()> {
    let config = HumosConfig::load().context("load config")?;
    log::info!("config: index={} socket={} scan_days={}",
        config.index_path.display(), config.socket_path.display(), config.scan_days);

    let indexer = Arc::new(
        KeywordIndexer::open(&config.index_path).context("open keyword index")?,
    );
    let handler = Arc::new(Handler::new(indexer.clone()));

    let scanner = Arc::new(Scanner::new(indexer.clone(), config.clone()));

    // Initial scan runs on the tokio runtime so it can proceed in parallel
    // with accepting early connections (which will return empty results
    // until scan completes).
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let scanner_for_scan = Arc::clone(&scanner);
    let handler_for_scan = Arc::clone(&handler);
    runtime.spawn(async move {
        log::info!("initial scan starting");
        match scanner_for_scan.scan_all() {
            Ok(count) => {
                log::info!("initial scan indexed {count} sessions");
                handler_for_scan.mark_indexed();
            }
            Err(e) => log::error!("initial scan failed: {e}"),
        }
    });

    // File watcher re-indexes on session file changes.
    let scanner_for_watch = Arc::clone(&scanner);
    let handler_for_watch = Arc::clone(&handler);
    let _debouncer_guard = spawn_watcher(scanner_for_watch, handler_for_watch)?;

    // IPC server
    runtime.block_on(async move {
        let server = IpcServer::bind(&config.socket_path, handler).await?;
        server.accept_loop().await
    })
}

fn spawn_watcher(
    scanner: Arc<Scanner>,
    handler: Arc<Handler>,
) -> Result<notify_debouncer_mini::Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>> {
    let scanner_for_cb = Arc::clone(&scanner);
    let mut debouncer = new_debouncer(
        Duration::from_secs(2),
        move |res: DebounceEventResult| match res {
            Ok(events) => {
                for ev in events {
                    match scanner_for_cb.index_path(&ev.path) {
                        Ok(true) => {
                            handler.mark_indexed();
                        }
                        Ok(false) => {}
                        Err(e) => log::warn!("re-index {} failed: {}", ev.path.display(), e),
                    }
                }
            }
            Err(err) => {
                log::warn!("watcher error: {err}");
            }
        },
    )?;

    for path in scanner.watch_paths() {
        if path.exists() {
            if let Err(e) = debouncer.watcher().watch(&path, RecursiveMode::Recursive) {
                log::warn!("watcher failed to register {}: {}", path.display(), e);
            } else {
                log::info!("watching {}", path.display());
            }
        }
    }

    Ok(debouncer)
}
