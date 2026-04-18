//! Index statistics aggregator. Reports chunk/file counts, source-type
//! breakdown, and search-log totals.

use crate::error::Result;
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct Stats {
    pub chunks: i64,
    pub files: i64,
    pub source_types: BTreeMap<String, i64>,
    pub search_log_total: i64,
    pub search_log_last_7d: i64,
}

pub async fn gather(pool: &SqlitePool) -> Result<Stats> {
    let (chunks,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM chunk_map").fetch_one(pool).await?;
    let (files,):  (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files").fetch_one(pool).await?;
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT source_type, COUNT(*) FROM chunk_map GROUP BY source_type"
    ).fetch_all(pool).await?;
    let source_types: BTreeMap<String, i64> = rows.into_iter().collect();
    let (total,):  (i64,) = sqlx::query_as("SELECT COUNT(*) FROM search_log").fetch_one(pool).await?;
    let (recent,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM search_log WHERE ts >= datetime('now', '-7 days')"
    ).fetch_one(pool).await?;
    Ok(Stats { chunks, files, source_types, search_log_total: total, search_log_last_7d: recent })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, indexer::{self, IndexRoot}};
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn reports_counts_and_source_types() {
        let dir = tempdir().unwrap();
        let mem = dir.path().join("memory");
        fs::create_dir_all(mem.join("projects")).unwrap();
        fs::write(mem.join("a.md"), "# A\ntext").unwrap();
        fs::write(mem.join("projects/b.md"), "# B\ntext").unwrap();
        let pool = db::open(&dir.path().join("s.db")).await.unwrap();
        indexer::reindex(&pool, &[IndexRoot { dir: mem, prefix: String::new() }]).await.unwrap();
        let s = gather(&pool).await.unwrap();
        assert_eq!(s.files, 2);
        assert!(s.chunks >= 2);
        assert_eq!(s.source_types.get("memory").copied().unwrap_or(0), 1);
        assert_eq!(s.source_types.get("project").copied().unwrap_or(0), 1);
    }
}
