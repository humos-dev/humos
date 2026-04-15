//! Tantivy schema definition and schema-version marker file.
//!
//! Version bumps trigger a full index rebuild. Bump CURRENT_VERSION whenever
//! field definitions change in incompatible ways.

use anyhow::Context;
use std::path::Path;
use tantivy::schema::{Schema, FAST, STORED, STRING, TEXT};

pub const CURRENT_VERSION: u32 = 1;

pub fn build_schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("id", STRING | STORED);
    builder.add_text_field("provider", STRING | STORED);
    builder.add_text_field("cwd", TEXT | STORED);
    builder.add_text_field("project", TEXT | STORED);
    builder.add_text_field("content", TEXT);
    builder.add_text_field("snippet", STORED);
    builder.add_date_field("modified_at", STORED | FAST);
    builder.build()
}

pub fn schema_version_path(index_dir: &Path) -> std::path::PathBuf {
    index_dir.join("SCHEMA_VERSION")
}

/// Read the recorded schema version. None if missing or unreadable.
pub fn read_version(index_dir: &Path) -> Option<u32> {
    let path = schema_version_path(index_dir);
    let text = std::fs::read_to_string(&path).ok()?;
    text.trim().parse::<u32>().ok()
}

pub fn write_version(index_dir: &Path, version: u32) -> anyhow::Result<()> {
    let path = schema_version_path(index_dir);
    std::fs::write(&path, version.to_string())
        .with_context(|| format!("write schema version to {}", path.display()))?;
    Ok(())
}

pub fn needs_rebuild(index_dir: &Path) -> bool {
    match read_version(index_dir) {
        Some(v) => v != CURRENT_VERSION,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn schema_has_expected_fields() {
        let s = build_schema();
        assert!(s.get_field("id").is_ok());
        assert!(s.get_field("provider").is_ok());
        assert!(s.get_field("cwd").is_ok());
        assert!(s.get_field("project").is_ok());
        assert!(s.get_field("content").is_ok());
        assert!(s.get_field("snippet").is_ok());
        assert!(s.get_field("modified_at").is_ok());
    }

    #[test]
    fn version_roundtrip() {
        let dir = TempDir::new().unwrap();
        assert!(!needs_rebuild(dir.path()));
        write_version(dir.path(), CURRENT_VERSION).unwrap();
        assert_eq!(read_version(dir.path()), Some(CURRENT_VERSION));
        assert!(!needs_rebuild(dir.path()));
        write_version(dir.path(), 999).unwrap();
        assert!(needs_rebuild(dir.path()));
    }
}
