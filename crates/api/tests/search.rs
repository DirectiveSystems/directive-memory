use axum::body::Body;
use axum::http::{Request, StatusCode};
use dm_api::build_router;
use dm_core::{config::Config, Core};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn search_returns_hits() {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    std::fs::create_dir_all(&mem).unwrap();
    std::fs::write(mem.join("a.md"), "# A\nfencing quote").unwrap();
    let mut cfg = Config::default();
    cfg.memory_dir = mem.clone();
    cfg.db_path    = dir.path().join("db.sqlite");
    cfg.api_key    = "k".into();
    let core = Core::open(cfg).await.unwrap();
    core.reindex().await.unwrap();
    let app = build_router(core);

    let resp = app.oneshot(
        Request::builder()
            .uri("/api/search?q=fencing&top_k=3")
            .header("x-api-key", "k").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &resp.into_body().collect().await.unwrap().to_bytes()
    ).unwrap();
    assert!(v["hits"].as_array().unwrap().len() >= 1);
}
