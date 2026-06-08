use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode };
use rt_renderer::camera::{Camera, CameraConfig, CameraUpdatePayload, Applicable};

use crate::{handlers::ErrorResponse, state::AppState};


pub async fn get_camera_handler(State(state): State<AppState>) -> Result<Json<CameraConfig>, StatusCode>{
    let camera_lock = state.camera.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let config = camera_lock.config;

    return Ok(Json(config))
}

#[axum::debug_handler(state = AppState)]
pub async fn update_camera_handler(State(state): State<AppState>, Json(payload): Json<CameraUpdatePayload>) -> Result<(StatusCode, Json<CameraConfig>), (StatusCode, Json<ErrorResponse>)> {
    let mut config = match state.camera.read() {
        Ok(camera_lock) => camera_lock.config,
        Err(_) => {
            let error_body = Json(ErrorResponse{
                error: "The global camera lock hs suffered a poisoning".to_string(),
            });
            return Err((StatusCode::INTERNAL_SERVER_ERROR, error_body));
        }
    };

    let original_width = config.image_width;
    let original_aspect_ratio = config.aspect_ratio;

    payload.apply_to(&mut config);

    if original_width != config.image_width || original_aspect_ratio != config.aspect_ratio {
        let error_body = Json(ErrorResponse{
            error: "The image width and aspect ratio cannot be updated".to_string(),
        });
        return Err((StatusCode::BAD_REQUEST, error_body));
    }

    let new_camera = Camera::new(config);

    match state.camera.write() {
        Ok(mut camera_lock) => {
            *camera_lock = Arc::new(new_camera);

            Ok((StatusCode::OK, Json(config)))
        },
        Err(_) => {
            let error_body = Json(ErrorResponse{
                error: "Critical failure while trying to write the new camera to memory.".to_string()
            });

            Err((StatusCode::INTERNAL_SERVER_ERROR, error_body))
        },
    }


}
