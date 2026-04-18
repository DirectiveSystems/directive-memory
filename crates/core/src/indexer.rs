use crate::{chunker, error::Result, source_type};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct IndexRoot {
    pub dir: PathBuf,
    /// Virtual prefix applied to relative paths (e.g. "vault/") — empty for the primary root.
    pub prefix: String,
}

#[derive(Debug, Default)]
pub struct IndexReport {
    pub files_indexed: usize,
    pub files_pruned: usize,
}

pub async fn reindex(pool: &SqlitePool, roots: &[IndexRoot]) -> Result<IndexReport> {
    let mut report = IndexReport::default();
    let mut live: HashSet<String> = HashSet::new();

    for root in roots {
        if !root.dir.exists() { continue; }
        for entry in WalkDir::new(&root.dir).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() { continue; }
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") { continue; }

            let rel_os = path.strip_prefix(&root.dir).unwrap();
            let rel = format!("{}{}", root.prefix, rel_os.to_string_lossy().replace('\\', "/"));
            live.insert(rel.clone());

            let mtime = mtime_of(path)?;
            if needs_reindex(pool, &rel, mtime).await? {
                index_file(pool, path, &rel, mtime).await?;
                report.files_indexed += 1;
            }
        }
    }

    let indexed: Vec<(String,)> = sqlx::query_as("SELECT path FROM files")
        .fetch_all(pool).await?;
    for (path,) in indexed {
        if !live.contains(&path) {
            delete_file(pool, &path).await?;
            report.files_pruned += 1;
        }
    }
    Ok(report)
}

fn mtime_of(path: &Path) -> Result<f64> {
    let meta = std::fs::metadata(path)?;
    let mtime = meta.modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| crate::error::CoreError::Other(e.to_string()))?
        .as_secs_f64();
    Ok(mtime)
}

async fn needs_reindex(pool: &SqlitePool, rel: &str, mtime: f64) -> Result<bool> {
    let row: Option<(f64,)> = sqlx::query_as("SELECT mtime FROM files WHERE path = ?1")
        .bind(rel).fetch_optional(pool).await?;
    Ok(match row { None => true, Some((old,)) => old < mtime })
}

/// Re-index a single file after a disk write. If the file no longer exists
/// on disk (e.g. an external delete) the entry is pruned from the index.
pub async fn reindex_path(pool: &SqlitePool, path: &Path, rel: &str) -> Result<()> {
    if path.exists() {
        let mtime = mtime_of(path)?;
        index_file(pool, path, rel, mtime).await
    } else {
        delete_file(pool, rel).await
    }
}

async fn index_file(pool: &SqlitePool, path: &Path, rel: &str, mtime: f64) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let chunks = chunker::parse_chunks(&text);
    let st = source_type::infer(rel).as_str();

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM chunks WHERE file = ?1").bind(rel).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM chunk_map WHERE file = ?1").bind(rel).execute(&mut *tx).await?;
    sqlx::query("INSERT OR REPLACE INTO files (path, mtime) VALUES (?1, ?2)")
        .bind(rel).bind(mtime).execute(&mut *tx).await?;
    for c in &chunks {
        sqlx::query("INSERT INTO chunks (file, heading, content) VALUES (?1, ?2, ?3)")
            .bind(rel).bind(&c.heading).bind(&c.content).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO chunk_map (file, heading, content, source_type) VALUES (?1, ?2, ?3, ?4)"
        )
        .bind(rel).bind(&c.heading).bind(&c.content).bind(st)
        .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn delete_file(pool: &SqlitePool, rel: &str) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM chunks WHERE file = ?1").bind(rel).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM chunk_map WHERE file = ?1").bind(rel).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM files WHERE path = ?1").bind(rel).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
