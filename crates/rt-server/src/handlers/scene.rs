use axum::{Json, extract::State, http::StatusCode};
use rt_core::dto::ScenePayload;

use crate::state::AppState;


pub async fn get_scene_handler(State(state): State<AppState>) -> Result<Json<ScenePayload>, StatusCode>{
    let scene_lock = state.scene_data.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(ref escena) = *scene_lock {
        Ok(Json(escena.clone())) // Devolvemos el espejo exacto y limpio
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
