//! Core facade. The single entry point API, MCP, and CLI layers wrap.

use crate::config::Config;
use crate::error::Result;
use crate::indexer::{self, IndexReport};
use crate::search::{self, SearchHit, SearchQuery};
use crate::stats::{self, Stats};
use crate::writeback;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct Core {
    pub config: Arc<Config>,
    pub pool: SqlitePool,
}

impl Core {
    pub async fn open(config: Config) -> anyhow::Result<Self> {
        let pool = crate::db::open(&config.db_path).await?;
        Ok(Self { config: Arc::new(config), pool })
    }

    pub async fn reindex(&self) -> Result<IndexReport> {
        indexer::reindex(&self.pool, &self.config.index_roots()).await
    }

    pub async fn search(&self, q: &SearchQuery) -> Result<Vec<SearchHit>> {
        search::search(&self.pool, q).await
    }

    pub async fn stats(&self) -> Result<Stats> { stats::gather(&self.pool).await }

    pub fn write_file(&self, rel_path: &str, content: &str, append: bool) -> Result<()> {
        writeback::write_file(&self.config.memory_dir, rel_path, content, append)
    }
    pub fn add_fact(&self, rel_path: &str, section: &str, fact: &str) -> Result<()> {
        writeback::add_fact(&self.config.memory_dir, rel_path, section, fact)
    }

    pub async fn list_files(&self) -> Result<Vec<(String, f64)>> {
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT path, mtime FROM files ORDER BY path"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub fn read_file(&self, rel_path: &str) -> Result<String> {
        let root = &self.config.memory_dir;
        let rel = rel_path.trim().trim_start_matches('/');
        if rel.contains("..") {
            return Err(crate::error::CoreError::InvalidPath(rel_path.into()));
        }
        let full = root.join(rel);
        Ok(std::fs::read_to_string(full)?)
    }
}
