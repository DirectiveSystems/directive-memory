use dm_core::{config::Config, search::SearchQuery, Core};
use tempfile::tempdir;

async fn fresh_core() -> (tempfile::TempDir, Core) {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    std::fs::create_dir_all(&mem).unwrap();
    let mut cfg = Config::default();
    cfg.memory_dir = mem;
    cfg.db_path = dir.path().join("db.sqlite");
    cfg.api_key = "k".into();
    let core = Core::open(cfg).await.unwrap();
    (dir, core)
}

#[tokio::test]
async fn write_file_makes_content_immediately_searchable() {
    let (_d, core) = fresh_core().await;
    // No explicit reindex — the write itself should index.
    core.write_file("notes.md", "# Topic\nfencing quote discussion", false)
        .await
        .unwrap();
    let hits = core
        .search(&SearchQuery {
            query: "fencing".into(),
            top_k: 5,
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(!hits.is_empty(), "write should be searchable without manual reindex");
    assert_eq!(hits[0].file, "notes.md");
}

#[tokio::test]
async fn add_fact_makes_content_immediately_searchable() {
    let (_d, core) = fresh_core().await;
    core.add_fact(
        "learnings.md",
        "## Patterns",
        "quadrillion flexible banana invariant",
    )
    .await
    .unwrap();
    let hits = core
        .search(&SearchQuery {
            query: "quadrillion banana".into(),
            top_k: 5,
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(!hits.is_empty(), "add_fact should be searchable without manual reindex");
    assert_eq!(hits[0].file, "learnings.md");
}

#[tokio::test]
async fn overwrite_replaces_previous_chunks() {
    let (_d, core) = fresh_core().await;
    core.write_file("note.md", "# A\noriginal fencing content", false)
        .await
        .unwrap();
    core.write_file("note.md", "# A\nbrand new hedgehog content", false)
        .await
        .unwrap();
    let fencing_hits = core
        .search(&SearchQuery {
            query: "fencing".into(),
            top_k: 5,
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(
        fencing_hits.is_empty(),
        "overwrite should have removed old chunks; got {:?}",
        fencing_hits
    );
    let hedgehog_hits = core
        .search(&SearchQuery {
            query: "hedgehog".into(),
            top_k: 5,
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(!hedgehog_hits.is_empty(), "new content should be searchable");
}
