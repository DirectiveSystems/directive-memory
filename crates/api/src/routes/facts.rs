use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use crate::{error::ApiError, state::AppState};
use super::files::OkResponse;

#[derive(Deserialize)]
pub struct FactBody {
    pub file: String,
    pub section: String,
    pub fact: String,
}

pub async fn add(
    State(state): State<AppState>,
    Json(body): Json<FactBody>,
) -> Result<Json<OkResponse>, ApiError> {
    state.core.add_fact(&body.file, &body.section, &body.fact)?;
    Ok(Json(OkResponse { ok: true, path: body.file }))
}
