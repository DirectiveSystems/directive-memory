use axum::body::Body;
use axum::http::{Request, StatusCode};
use dm_api::build_router;
use dm_core::{config::Config, Core};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::util::ServiceExt;

async fn setup() -> (tempfile::TempDir, axum::Router) {
    let dir = tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.memory_dir = dir.path().join("memory");
    cfg.db_path    = dir.path().join("db.sqlite");
    cfg.api_key    = "test-key".into();
    std::fs::create_dir_all(&cfg.memory_dir).unwrap();
    let core = Core::open(cfg).await.unwrap();
    let router = build_router(core);
    (dir, router)
}

#[tokio::test]
async fn health_returns_ok_without_auth() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"ok");
}

#[tokio::test]
async fn api_routes_reject_missing_api_key() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(
        Request::builder().uri("/api/stats").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn root_serves_index_html() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(
        axum::http::Request::builder().uri("/").body(axum::body::Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let body = std::str::from_utf8(&bytes).unwrap();
    assert!(body.contains("Directive Memory"));
}
