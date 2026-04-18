use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use crate::{error::ApiError, state::AppState};

#[derive(Serialize)]
pub struct FileEntry { pub path: String, pub mtime: f64 }

#[derive(Serialize)]
pub struct ListResponse { pub files: Vec<FileEntry> }

pub async fn list(State(state): State<AppState>) -> Result<Json<ListResponse>, ApiError> {
    let rows = state.core.list_files().await?;
    Ok(Json(ListResponse {
        files: rows.into_iter().map(|(path, mtime)| FileEntry { path, mtime }).collect(),
    }))
}

#[derive(Serialize)]
pub struct FileContent { pub path: String, pub content: String }

pub async fn read(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<Json<FileContent>, ApiError> {
    let content = state.core.read_file(&path)?;
    Ok(Json(FileContent { path, content }))
}

#[derive(Deserialize)]
pub struct WriteBody { pub content: String }

#[derive(Serialize)]
pub struct OkResponse { pub ok: bool, pub path: String }

pub async fn write(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Json(body): Json<WriteBody>,
) -> Result<Json<OkResponse>, ApiError> {
    state.core.write_file(&path, &body.content, false).await?;
    Ok(Json(OkResponse { ok: true, path }))
}

pub async fn append(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Json(body): Json<WriteBody>,
) -> Result<Json<OkResponse>, ApiError> {
    state.core.write_file(&path, &body.content, true).await?;
    Ok(Json(OkResponse { ok: true, path }))
}
