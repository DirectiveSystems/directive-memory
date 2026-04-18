use dm_core::db;
use tempfile::tempdir;

#[tokio::test]
async fn open_creates_file_and_runs_migrations() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = db::open(&db_path).await.expect("open pool");
    for name in ["files", "chunks", "chunk_map", "search_log", "meta"] {
        let row: (String,) = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type IN ('table','view') AND name = ?1"
        )
        .bind(name).fetch_one(&pool).await
        .unwrap_or_else(|e| panic!("table {name} missing: {e}"));
        assert_eq!(row.0, name);
    }
    assert!(db_path.exists());
}

#[tokio::test]
async fn fts5_match_returns_inserted_chunks() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("fts.db");
    let pool = db::open(&db_path).await.expect("open pool");

    sqlx::query("INSERT INTO chunks (file, heading, content) VALUES (?1, ?2, ?3)")
        .bind("alpha.md").bind("Intro").bind("fencing quote was 3638 dollars")
        .execute(&pool).await.unwrap();

    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT file, heading FROM chunks WHERE chunks MATCH ?1"
    )
    .bind("fencing").fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "alpha.md");
    assert_eq!(rows[0].1, "Intro");
}
