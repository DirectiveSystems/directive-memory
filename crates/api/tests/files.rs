use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use dm_api::build_router;
use dm_core::{config::Config, Core};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::util::ServiceExt;

async fn setup() -> (tempfile::TempDir, axum::Router) {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    std::fs::create_dir_all(&mem).unwrap();
    std::fs::write(mem.join("a.md"), "# A\nhello").unwrap();
    let mut cfg = Config::default();
    cfg.memory_dir = mem.clone();
    cfg.db_path    = dir.path().join("db.sqlite");
    cfg.api_key    = "k".into();
    let core = Core::open(cfg).await.unwrap();
    core.reindex().await.unwrap();
    (dir, build_router(core))
}

fn req(method: Method, uri: &str, body: &str) -> Request<Body> {
    Request::builder().method(method).uri(uri)
        .header("x-api-key", "k")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn list_files_returns_indexed_files() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(req(Method::GET, "/api/files", "")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &resp.into_body().collect().await.unwrap().to_bytes()
    ).unwrap();
    assert!(v["files"].as_array().unwrap().iter().any(|f| f["path"] == "a.md"));
}

#[tokio::test]
async fn read_file_returns_content() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(req(Method::GET, "/api/files/a.md", "")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &resp.into_body().collect().await.unwrap().to_bytes()
    ).unwrap();
    assert!(v["content"].as_str().unwrap().contains("hello"));
}

#[tokio::test]
async fn write_then_read_roundtrip() {
    let (_d, app) = setup().await;
    let resp = app.clone().oneshot(req(
        Method::POST, "/api/files/new.md", r##"{"content":"# New\nbody"}"##
    )).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app.oneshot(req(Method::GET, "/api/files/new.md", "")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &resp.into_body().collect().await.unwrap().to_bytes()
    ).unwrap();
    assert!(v["content"].as_str().unwrap().contains("# New"));
}

#[tokio::test]
async fn append_adds_to_file() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(req(
        Method::PATCH, "/api/files/a.md", r#"{"content":"extra"}"#
    )).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn rejects_path_traversal() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(req(
        Method::POST, "/api/files/..%2Fetc%2Fpasswd.md", r#"{"content":"x"}"#
    )).await.unwrap();
    assert!(resp.status().is_client_error());
}
