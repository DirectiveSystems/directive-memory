use axum::body::Body;
use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/"]
struct Assets;

pub async fn serve_root() -> Response { serve_path("index.html".into()).await }

pub async fn serve(Path(path): Path<String>) -> Response { serve_path(path).await }

async fn serve_path(path: String) -> Response {
    // SPA fallback: unknown paths re-serve index.html so client-side routes
    // work if Task 20 grows one. For the placeholder page this is harmless.
    let asset = Assets::get(&path).or_else(|| Assets::get("index.html"));
    match asset {
        Some(file) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(file.data.into_owned()))
                .unwrap()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

pub fn router() -> axum::Router {
    axum::Router::new().route("/", get(serve_root)).route("/*path", get(serve))
}
