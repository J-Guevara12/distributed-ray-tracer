use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use rt_renderer::camera::{Applicable, Camera, CameraConfig, CameraUpdatePayload};

use crate::{handlers::ErrorResponse, state::AppState};

pub async fn get_camera_handler(
    State(state): State<AppState>,
) -> Result<Json<CameraConfig>, StatusCode> {
    let camera_lock = state.camera.read();

    let config = camera_lock.config;

    Ok(Json(config))
}

pub async fn update_camera_handler(
    State(state): State<AppState>,
    Json(payload): Json<CameraUpdatePayload>,
) -> Result<(StatusCode, Json<CameraConfig>), (StatusCode, Json<ErrorResponse>)> {
    let mut config = state.camera.read().config; 

    let original_width = config.image_width;
    let original_aspect_ratio = config.aspect_ratio;

    payload.apply_to(&mut config);

    if original_width != config.image_width || original_aspect_ratio != config.aspect_ratio {
        let error_body = Json(ErrorResponse {
            error: "The image width and aspect ratio cannot be updated".to_string(),
        });
        return Err((StatusCode::BAD_REQUEST, error_body));
    }

    let new_camera = Camera::new(config);

    let mut camera_lock = state.camera.write();
    *camera_lock = Arc::new(new_camera);

    Ok((StatusCode::OK, Json(config)))
}
