use dm_core::{db, indexer::{self, IndexRoot}};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn indexes_new_files_and_detects_mtime_changes() {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    fs::create_dir_all(&mem).unwrap();
    fs::write(mem.join("alpha.md"), "# Alpha\ncontent about alpha").unwrap();
    fs::write(mem.join("beta.md"),  "# Beta\ncontent about beta").unwrap();

    let pool = db::open(&dir.path().join("idx.db")).await.unwrap();
    let roots = vec![IndexRoot { dir: mem.clone(), prefix: String::new() }];

    let report = indexer::reindex(&pool, &roots).await.unwrap();
    assert_eq!(report.files_indexed, 2);
    assert_eq!(report.files_pruned, 0);

    let noop = indexer::reindex(&pool, &roots).await.unwrap();
    assert_eq!(noop.files_indexed, 0);

    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(mem.join("alpha.md"), "# Alpha\nbrand new content").unwrap();
    let touched = indexer::reindex(&pool, &roots).await.unwrap();
    assert_eq!(touched.files_indexed, 1);

    fs::remove_file(mem.join("beta.md")).unwrap();
    let pruned = indexer::reindex(&pool, &roots).await.unwrap();
    assert_eq!(pruned.files_pruned, 1);
}

#[tokio::test]
async fn applies_prefix_for_external_roots() {
    let dir = tempdir().unwrap();
    let vault = dir.path().join("vault_src");
    fs::create_dir_all(&vault).unwrap();
    fs::write(vault.join("note.md"), "# Note\ntext").unwrap();

    let pool = db::open(&dir.path().join("idx.db")).await.unwrap();
    let roots = vec![IndexRoot { dir: vault.clone(), prefix: "vault/".into() }];
    indexer::reindex(&pool, &roots).await.unwrap();

    let (path,): (String,) = sqlx::query_as("SELECT path FROM files LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(path, "vault/note.md");

    let (source_type,): (String,) = sqlx::query_as("SELECT source_type FROM chunk_map LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(source_type, "vault");
}
