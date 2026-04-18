use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use dm_api::build_router;
use dm_core::{config::Config, Core};
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn add_fact_writes_bullet() {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    std::fs::create_dir_all(&mem).unwrap();
    let mut cfg = Config::default();
    cfg.memory_dir = mem.clone();
    cfg.db_path    = dir.path().join("db.sqlite");
    cfg.api_key    = "k".into();
    let core = Core::open(cfg).await.unwrap();
    let app = build_router(core);

    let resp = app.oneshot(
        Request::builder().method(Method::POST).uri("/api/facts")
            .header("x-api-key", "k").header("content-type", "application/json")
            .body(Body::from(
                r###"{"file":"learnings.md","section":"## Patterns","fact":"use sqlx"}"###
            )).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = std::fs::read_to_string(mem.join("learnings.md")).unwrap();
    assert!(body.contains("- use sqlx"));
}
