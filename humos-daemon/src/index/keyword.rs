//! KeywordIndexer: tantivy-backed full-text search over session content.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Value};
use tantivy::{
    doc, DateTime as TantivyDateTime, Index, IndexReader, IndexWriter, ReloadPolicy,
    TantivyDocument, Term,
};

use super::redact::Redactor;
use super::schema::{build_schema, needs_rebuild, write_version, CURRENT_VERSION};
use super::{IndexableSession, SearchResult, SessionIndexer};

const INDEX_HEAP_SIZE: usize = 50_000_000;
const SNIPPET_MAX_CHARS: usize = 120;

pub struct KeywordIndexer {
    index: Index,
    writer: Mutex<IndexWriter>,
    reader: IndexReader,
    redactor: Redactor,
    redact_enabled: bool,
    index_dir: PathBuf,
}

impl KeywordIndexer {
    pub fn open(index_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(index_dir)
            .with_context(|| format!("create index dir {}", index_dir.display()))?;

        if needs_rebuild(index_dir) {
            log::warn!("schema version mismatch, rebuilding index at {}", index_dir.display());
            for entry in std::fs::read_dir(index_dir).with_context(|| "read index dir for rebuild")? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let _ = std::fs::remove_file(&path);
                } else if path.is_dir() {
                    let _ = std::fs::remove_dir_all(&path);
                }
            }
        }

        let schema = build_schema();
        let index = match Index::open_in_dir(index_dir) {
            Ok(idx) => idx,
            Err(_) => Index::create_in_dir(index_dir, schema.clone())
                .with_context(|| format!("create index in {}", index_dir.display()))?,
        };

        write_version(index_dir, CURRENT_VERSION)?;

        let writer = index
            .writer(INDEX_HEAP_SIZE)
            .with_context(|| "acquire IndexWriter (another daemon may be running)")?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .context("build IndexReader")?;

        let redact_enabled = !Redactor::is_disabled_by_env();

        Ok(Self {
            index,
            writer: Mutex::new(writer),
            reader,
            redactor: Redactor::new(),
            redact_enabled,
            index_dir: index_dir.to_path_buf(),
        })
    }

    pub fn index_dir(&self) -> &Path {
        &self.index_dir
    }

    fn prepare_content(&self, raw: &str) -> String {
        if self.redact_enabled {
            self.redactor.redact(raw)
        } else {
            raw.to_string()
        }
    }

    fn field(&self, name: &str) -> anyhow::Result<tantivy::schema::Field> {
        self.index
            .schema()
            .get_field(name)
            .with_context(|| format!("schema missing field {name}"))
    }

    fn result_from_doc(&self, doc: &TantivyDocument, score: f32) -> Result<SearchResult> {
        let id = str_field(doc, self.field("id")?);
        let provider = str_field(doc, self.field("provider")?);
        let cwd = str_field(doc, self.field("cwd")?);
        let project = str_field(doc, self.field("project")?);
        let snippet = str_field(doc, self.field("snippet")?);
        let modified_at = date_field(doc, self.field("modified_at")?)
            .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap());

        Ok(SearchResult {
            id,
            provider,
            cwd,
            project,
            snippet,
            modified_at,
            score,
        })
    }
}

impl SessionIndexer for KeywordIndexer {
    fn index(&self, session: &IndexableSession) -> Result<()> {
        let id_field = self.field("id")?;
        let provider_field = self.field("provider")?;
        let cwd_field = self.field("cwd")?;
        let project_field = self.field("project")?;
        let content_field = self.field("content")?;
        let snippet_field = self.field("snippet")?;
        let modified_field = self.field("modified_at")?;

        let content = self.prepare_content(&session.content);
        let snippet_raw = super::truncate_snippet(&content, SNIPPET_MAX_CHARS);
        let modified_ts = TantivyDateTime::from_timestamp_secs(session.modified_at.timestamp());

        let mut writer = self.writer.lock().map_err(|e| anyhow::anyhow!("writer lock poisoned: {e}"))?;
        writer.delete_term(Term::from_field_text(id_field, &session.id));
        writer.add_document(doc!(
            id_field => session.id.as_str(),
            provider_field => session.provider.as_str(),
            cwd_field => session.cwd.as_str(),
            project_field => session.project.as_str(),
            content_field => content,
            snippet_field => snippet_raw,
            modified_field => modified_ts,
        ))?;
        writer.commit()?;
        drop(writer);
        self.reader.reload().context("reload reader after index")?;
        Ok(())
    }

    fn delete(&self, session_id: &str) -> Result<()> {
        let id_field = self.field("id")?;
        let mut writer = self.writer.lock().map_err(|e| anyhow::anyhow!("writer lock poisoned: {e}"))?;
        writer.delete_term(Term::from_field_text(id_field, session_id));
        writer.commit()?;
        drop(writer);
        self.reader.reload().context("reload reader after delete")?;
        Ok(())
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        if query.trim().is_empty() {
            return self.search_recent(limit);
        }
        let searcher = self.reader.searcher();
        let content_field = self.field("content")?;
        let project_field = self.field("project")?;
        let cwd_field = self.field("cwd")?;
        let parser = QueryParser::for_index(
            &self.index,
            vec![content_field, project_field, cwd_field],
        );
        let parsed = parser
            .parse_query(query)
            .with_context(|| format!("parse query {query:?}"))?;
        let top = searcher.search(&parsed, &TopDocs::with_limit(limit))?;
        let mut out = Vec::with_capacity(top.len());
        for (score, addr) in top {
            let doc: TantivyDocument = searcher.doc(addr)?;
            out.push(self.result_from_doc(&doc, score)?);
        }
        Ok(out)
    }

    fn search_by_cwd(&self, cwd: &str, limit: usize) -> Result<Vec<SearchResult>> {
        if cwd.trim().is_empty() {
            return Ok(Vec::new());
        }
        let searcher = self.reader.searcher();
        let cwd_field = self.field("cwd")?;
        let parser = QueryParser::for_index(&self.index, vec![cwd_field]);
        let parsed = parser
            .parse_query(&format!("\"{}\"", cwd.replace('\"', "")))
            .with_context(|| format!("parse cwd query {cwd:?}"))?;
        let top = searcher.search(&parsed, &TopDocs::with_limit(limit))?;
        let mut out = Vec::with_capacity(top.len());
        for (score, addr) in top {
            let doc: TantivyDocument = searcher.doc(addr)?;
            let result = self.result_from_doc(&doc, score)?;
            // Cwd parser is TEXT so partial tokens match. Confirm exact prefix match.
            if result.cwd == cwd || result.cwd.starts_with(cwd) {
                out.push(result);
            }
        }
        Ok(out)
    }

    fn total_count(&self) -> Result<u64> {
        let searcher = self.reader.searcher();
        Ok(searcher.num_docs())
    }

    fn commit(&self) -> Result<()> {
        let mut writer = self.writer.lock().map_err(|e| anyhow::anyhow!("writer lock poisoned: {e}"))?;
        writer.commit()?;
        drop(writer);
        self.reader.reload().context("reload reader after commit")?;
        Ok(())
    }
}

fn str_field(doc: &TantivyDocument, field: Field) -> String {
    doc.get_first(field)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn date_field(doc: &TantivyDocument, field: Field) -> Option<DateTime<Utc>> {
    let v = doc.get_first(field)?;
    let ts = v.as_datetime()?;
    let secs = ts.into_timestamp_secs();
    Utc.timestamp_opt(secs, 0).single()
}

impl KeywordIndexer {
    fn search_recent(&self, limit: usize) -> Result<Vec<SearchResult>> {
        use tantivy::query::AllQuery;
        let searcher = self.reader.searcher();
        let top = searcher.search(&AllQuery, &TopDocs::with_limit(limit))?;
        let mut out = Vec::with_capacity(top.len());
        for (score, addr) in top {
            let doc: TantivyDocument = searcher.doc(addr)?;
            out.push(self.result_from_doc(&doc, score)?);
        }
        out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample(id: &str, cwd: &str, content: &str) -> IndexableSession {
        IndexableSession {
            id: id.into(),
            provider: "claude".into(),
            cwd: cwd.into(),
            project: cwd.split('/').last().unwrap_or("").into(),
            content: content.into(),
            started_at: Utc::now(),
            modified_at: Utc::now(),
        }
    }

    #[test]
    fn index_and_search_content() {
        let dir = TempDir::new().unwrap();
        let idx = KeywordIndexer::open(dir.path()).unwrap();
        idx.index(&sample("a", "/tmp/a", "implementing daemon with tantivy")).unwrap();
        idx.index(&sample("b", "/tmp/b", "writing React hooks for dashboard")).unwrap();
        let results = idx.search("tantivy", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "a");
    }

    #[test]
    fn search_by_cwd_filters_correctly() {
        let dir = TempDir::new().unwrap();
        let idx = KeywordIndexer::open(dir.path()).unwrap();
        idx.index(&sample("a", "/Users/bolu/humos", "alpha")).unwrap();
        idx.index(&sample("b", "/Users/bolu/humos", "beta")).unwrap();
        idx.index(&sample("c", "/Users/bolu/other", "gamma")).unwrap();
        let results = idx.search_by_cwd("/Users/bolu/humos", 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn delete_removes_session() {
        let dir = TempDir::new().unwrap();
        let idx = KeywordIndexer::open(dir.path()).unwrap();
        idx.index(&sample("a", "/tmp/a", "quick brown fox")).unwrap();
        assert_eq!(idx.total_count().unwrap(), 1);
        idx.delete("a").unwrap();
        assert_eq!(idx.total_count().unwrap(), 0);
    }

    #[test]
    fn reindex_same_id_replaces_document() {
        let dir = TempDir::new().unwrap();
        let idx = KeywordIndexer::open(dir.path()).unwrap();
        idx.index(&sample("a", "/tmp/a", "first")).unwrap();
        idx.index(&sample("a", "/tmp/a", "second")).unwrap();
        assert_eq!(idx.total_count().unwrap(), 1);
        let results = idx.search("second", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn snippet_is_truncated() {
        let dir = TempDir::new().unwrap();
        let idx = KeywordIndexer::open(dir.path()).unwrap();
        let long = "snippet ".repeat(100);
        idx.index(&sample("snippet-case", "/tmp/a", &long)).unwrap();
        let results = idx.search("snippet", 10).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].snippet.chars().count() <= 125);
    }

    #[test]
    fn secret_patterns_are_redacted_at_index_time() {
        std::env::remove_var("HUMOS_INDEX_REDACT");
        let dir = TempDir::new().unwrap();
        let idx = KeywordIndexer::open(dir.path()).unwrap();
        idx.index(&sample("a", "/tmp/a", "my key is sk-ant-api03-abcdefghijklmnopqrstuvwxyz123456")).unwrap();
        let results = idx.search("REDACTED", 10).unwrap();
        assert_eq!(results.len(), 1);
        let direct = idx.search("sk-ant-api03", 10).unwrap();
        assert!(direct.is_empty());
    }

    #[test]
    fn empty_query_returns_recent_sessions() {
        let dir = TempDir::new().unwrap();
        let idx = KeywordIndexer::open(dir.path()).unwrap();
        for i in 0..5 {
            idx.index(&sample(&format!("s{i}"), "/tmp", &format!("content {i}"))).unwrap();
        }
        let results = idx.search("", 3).unwrap();
        assert_eq!(results.len(), 3);
    }
}
