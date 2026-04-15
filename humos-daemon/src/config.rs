//! Config file parser for `~/.humOS/config.toml`.
//!
//! A missing config file is not an error, defaults are used. Parse errors
//! fail loudly so users don't silently get unexpected behavior.

use anyhow::Context;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct HumosConfig {
    #[serde(default)]
    pub exclude_cwds: Vec<String>,

    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    #[serde(default)]
    pub disable_project_brain: bool,

    #[serde(default = "default_index_path")]
    pub index_path: PathBuf,

    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,

    /// Days of session history to index at startup. 7 matches the app.
    #[serde(default = "default_scan_days")]
    pub scan_days: u64,
}

impl Default for HumosConfig {
    fn default() -> Self {
        Self {
            exclude_cwds: Vec::new(),
            exclude_patterns: Vec::new(),
            disable_project_brain: false,
            index_path: default_index_path(),
            socket_path: default_socket_path(),
            scan_days: default_scan_days(),
        }
    }
}

fn default_index_path() -> PathBuf {
    humos_home().join("index")
}

fn default_socket_path() -> PathBuf {
    humos_home().join("daemon.sock")
}

fn default_scan_days() -> u64 {
    7
}

pub fn humos_home() -> PathBuf {
    dirs::home_dir()
        .expect("home directory")
        .join(".humOS")
}

pub fn config_path() -> PathBuf {
    humos_home().join("config.toml")
}

impl HumosConfig {
    pub fn load() -> anyhow::Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read config {}", path.display()))?;
        let config: Self = toml::from_str(&text)
            .with_context(|| format!("parse config {}", path.display()))?;
        Ok(config)
    }

    /// Return true if the given cwd should be indexed.
    /// False if it matches any exclude rule.
    pub fn should_index(&self, cwd: &str) -> bool {
        for excluded in &self.exclude_cwds {
            if cwd.starts_with(excluded) {
                return false;
            }
        }
        for pattern in &self.exclude_patterns {
            if let Ok(pat) = glob::Pattern::new(pattern) {
                if pat.matches(cwd) {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_index_everything() {
        let c = HumosConfig::default();
        assert!(c.should_index("/Users/bolu/anywhere"));
    }

    #[test]
    fn exclude_cwd_prefix() {
        let c = HumosConfig {
            exclude_cwds: vec!["/Users/bolu/company-x".into()],
            ..HumosConfig::default()
        };
        assert!(!c.should_index("/Users/bolu/company-x/src"));
        assert!(c.should_index("/Users/bolu/personal"));
    }

    #[test]
    fn exclude_pattern_glob() {
        let c = HumosConfig {
            exclude_patterns: vec!["**/company-x/**".into()],
            ..HumosConfig::default()
        };
        assert!(!c.should_index("/Users/bolu/work/company-x/src"));
        assert!(c.should_index("/Users/bolu/personal/humos"));
    }

    #[test]
    fn parses_minimal_toml() {
        let text = r#"
exclude_cwds = ["/tmp"]
disable_project_brain = true
"#;
        let c: HumosConfig = toml::from_str(text).unwrap();
        assert_eq!(c.exclude_cwds, vec!["/tmp".to_string()]);
        assert!(c.disable_project_brain);
        assert_eq!(c.scan_days, 7);
    }
}
