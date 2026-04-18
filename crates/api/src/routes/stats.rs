use axum::extract::State;
use axum::Json;
use serde::Serialize;
use crate::{error::ApiError, state::AppState};

pub async fn stats(State(state): State<AppState>)
    -> Result<Json<dm_core::stats::Stats>, ApiError>
{
    Ok(Json(state.core.stats().await?))
}

#[derive(Serialize)]
pub struct ReindexResponse { pub files_indexed: usize, pub files_pruned: usize }

pub async fn reindex(State(state): State<AppState>)
    -> Result<Json<ReindexResponse>, ApiError>
{
    let r = state.core.reindex().await?;
    Ok(Json(ReindexResponse { files_indexed: r.files_indexed, files_pruned: r.files_pruned }))
}
