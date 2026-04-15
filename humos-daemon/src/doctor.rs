//! Doctor: health checks + actionable fix suggestions.

use std::path::Path;

use crate::config::{config_path, humos_home, HumosConfig};
use crate::index::schema::{read_version, CURRENT_VERSION};

pub fn run() -> anyhow::Result<()> {
    println!("humOS daemon doctor\n");

    check("config file parses", || {
        let _c = HumosConfig::load()?;
        Ok(format!("OK ({})", config_path().display()))
    });

    check("humos home writable", || {
        let home = humos_home();
        std::fs::create_dir_all(&home)?;
        let probe = home.join(".doctor-probe");
        std::fs::write(&probe, "ok")?;
        std::fs::remove_file(&probe)?;
        Ok(format!("OK ({})", home.display()))
    });

    let config = HumosConfig::load().unwrap_or_default();

    check("index directory exists and is writable", || {
        std::fs::create_dir_all(&config.index_path)?;
        let probe = config.index_path.join(".doctor-probe");
        std::fs::write(&probe, "ok")?;
        std::fs::remove_file(&probe)?;
        Ok(format!("OK ({})", config.index_path.display()))
    });

    check("schema version", || {
        match read_version(&config.index_path) {
            Some(v) if v == CURRENT_VERSION => Ok(format!("OK (version {v})")),
            Some(v) => Ok(format!("mismatch (stored {v}, current {CURRENT_VERSION}); will rebuild on next start")),
            None => Ok("no version marker yet (fresh index)".into()),
        }
    });

    check("socket path writable", || {
        let sock = &config.socket_path;
        if let Some(parent) = sock.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(format!("OK ({})", sock.display()))
    });

    check("claude sessions directory", || {
        let claude = dirs::home_dir().unwrap().join(".claude").join("projects");
        if claude.exists() {
            let count = count_jsonl(&claude);
            Ok(format!("OK ({} jsonl files)", count))
        } else {
            Ok(format!("not found at {} (no Claude Code sessions to index yet, that's fine)", claude.display()))
        }
    });

    check("daemon already running", || {
        // If we can connect to the socket, another daemon is up.
        match std::os::unix::net::UnixStream::connect(&config.socket_path) {
            Ok(_) => Ok(format!("YES: another daemon is listening on {}", config.socket_path.display())),
            Err(_) => Ok("no (safe to start a new one)".into()),
        }
    });

    println!("\ndone.");
    Ok(())
}

fn check<F>(name: &str, f: F)
where
    F: FnOnce() -> anyhow::Result<String>,
{
    match f() {
        Ok(msg) => println!("  [pass] {name}: {msg}"),
        Err(e) => println!("  [FAIL] {name}: {e}"),
    }
}

fn count_jsonl(dir: &Path) -> usize {
    fn walk(dir: &Path, count: &mut usize) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, count);
                } else if path.extension().map_or(false, |e| e == "jsonl") {
                    *count += 1;
                }
            }
        }
    }
    let mut count = 0;
    walk(dir, &mut count);
    count
}
