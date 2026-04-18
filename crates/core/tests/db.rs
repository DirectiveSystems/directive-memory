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
