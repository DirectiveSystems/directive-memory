use axum::extract::{Query, State};
use axum::Json;
use dm_core::search::{SearchHit, SearchQuery};
use serde::{Deserialize, Serialize};
use crate::{error::ApiError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: String,
    #[serde(default = "default_top_k")]
    pub top_k: i64,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub file_prefix: Option<String>,
}
fn default_top_k() -> i64 { 5 }

#[derive(Serialize)]
pub struct SearchResponse { pub query: String, pub hits: Vec<SearchHit> }

pub async fn handler(
    State(state): State<AppState>,
    Query(p): Query<SearchParams>,
) -> Result<Json<SearchResponse>, ApiError> {
    let hits = state.core.search(&SearchQuery {
        query: p.q.clone(),
        top_k: p.top_k,
        filter_file: p.file_prefix.unwrap_or_default(),
        filter_source_type: p.source_type,
    }).await?;
    Ok(Json(SearchResponse { query: p.q, hits }))
}
