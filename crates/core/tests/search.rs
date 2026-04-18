use dm_core::{db, indexer::{self, IndexRoot}, search::{self, SearchQuery}};
use std::fs;
use tempfile::tempdir;

async fn setup() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    fs::create_dir_all(&mem).unwrap();
    fs::write(mem.join("alpha.md"), "# Alpha\nfencing quote was 3638 dollars").unwrap();
    fs::write(mem.join("beta.md"),  "# Beta\nsomething else entirely").unwrap();

    let projects = mem.join("projects");
    fs::create_dir_all(&projects).unwrap();
    fs::write(projects.join("sift.md"), "# Sift\nfencing discussion").unwrap();

    let pool = db::open(&dir.path().join("s.db")).await.unwrap();
    indexer::reindex(&pool, &[IndexRoot { dir: mem, prefix: String::new() }]).await.unwrap();
    (dir, pool)
}

#[tokio::test]
async fn bm25_finds_matching_chunks() {
    let (_d, pool) = setup().await;
    let hits = search::search(&pool, &SearchQuery {
        query: "fencing".into(), top_k: 5, ..Default::default()
    }).await.unwrap();
    assert!(!hits.is_empty());
    assert!(hits.iter().any(|h| h.file == "alpha.md"));
}

#[tokio::test]
async fn filter_by_file_prefix() {
    let (_d, pool) = setup().await;
    let hits = search::search(&pool, &SearchQuery {
        query: "fencing".into(), top_k: 5,
        filter_file: "projects/".into(), ..Default::default()
    }).await.unwrap();
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|h| h.file.starts_with("projects/")));
}

#[tokio::test]
async fn filter_by_source_type() {
    let (_d, pool) = setup().await;
    let hits = search::search(&pool, &SearchQuery {
        query: "fencing".into(), top_k: 5,
        filter_source_type: Some("project".into()), ..Default::default()
    }).await.unwrap();
    assert!(hits.iter().all(|h| h.file.starts_with("projects/")));
}

#[tokio::test]
async fn sanitizes_punctuation_in_query() {
    let (_d, pool) = setup().await;
    let hits = search::search(&pool, &SearchQuery {
        query: "fencing? \"quote\"".into(), top_k: 5, ..Default::default()
    }).await.unwrap();
    assert!(!hits.is_empty());
}

#[tokio::test]
async fn empty_query_returns_empty() {
    let (_d, pool) = setup().await;
    let hits = search::search(&pool, &SearchQuery {
        query: "   ".into(), top_k: 5, ..Default::default()
    }).await.unwrap();
    assert!(hits.is_empty());
}

#[tokio::test]
async fn newer_files_rank_higher_on_equal_match() {
    use dm_core::{db, indexer::{self, IndexRoot}, search::{self, SearchQuery}};
    use std::fs;
    use tempfile::tempdir;
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    fs::create_dir_all(&mem).unwrap();
    fs::write(mem.join("old.md"), "# Old\nfencing discussion topic").unwrap();
    let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(180 * 86400);
    filetime::set_file_mtime(mem.join("old.md"),
        filetime::FileTime::from_system_time(old_time)).unwrap();
    fs::write(mem.join("new.md"), "# New\nfencing discussion topic").unwrap();

    let pool = db::open(&dir.path().join("t.db")).await.unwrap();
    indexer::reindex(&pool, &[IndexRoot { dir: mem, prefix: String::new() }]).await.unwrap();
    let hits = search::search(&pool, &SearchQuery {
        query: "fencing discussion".into(), top_k: 5, ..Default::default()
    }).await.unwrap();
    assert_eq!(hits[0].file, "new.md");
}

#[tokio::test]
async fn search_appends_row_to_log() {
    use dm_core::{db, indexer::{self, IndexRoot}, search::{self, SearchQuery}};
    use std::fs;
    use tempfile::tempdir;
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    fs::create_dir_all(&mem).unwrap();
    fs::write(mem.join("a.md"), "# A\nfencing").unwrap();
    let pool = db::open(&dir.path().join("l.db")).await.unwrap();
    indexer::reindex(&pool, &[IndexRoot { dir: mem, prefix: String::new() }]).await.unwrap();
    let before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM search_log")
        .fetch_one(&pool).await.unwrap();
    let _ = search::search(&pool, &SearchQuery {
        query: "fencing".into(), top_k: 3, ..Default::default()
    }).await.unwrap();
    let after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM search_log")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(after.0, before.0 + 1);
}
