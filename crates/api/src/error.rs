use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self { status, message: message.into() }
    }
}

impl From<dm_core::CoreError> for ApiError {
    fn from(err: dm_core::CoreError) -> Self {
        use dm_core::CoreError::*;
        match err {
            InvalidPath(p) => ApiError::new(StatusCode::BAD_REQUEST, format!("invalid path: {p}")),
            Io(e) if e.kind() == std::io::ErrorKind::NotFound =>
                ApiError::new(StatusCode::NOT_FOUND, "file not found"),
            other => ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
