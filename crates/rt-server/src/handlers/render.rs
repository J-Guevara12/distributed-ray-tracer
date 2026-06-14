use std::sync::{Arc, atomic::Ordering};

use axum::{Json, extract::State, http::StatusCode};
use rt_renderer::{render::render_scene, tracers::PathTracer};

use crate::{handlers::ErrorResponse, state::AppState};

#[derive(serde::Serialize)]
pub struct SuccessMesage {
    pub message: String,
}

#[derive(serde::Deserialize)]
pub struct CreateJobPayload {
    pub tile_size: Option<u32>,
    pub max_depth: Option<u32>,
}

pub async fn post_render(
    State(state): State<AppState>,
    Json(payload): Json<CreateJobPayload>,
) -> Result<(StatusCode, Json<SuccessMesage>), (StatusCode, Json<ErrorResponse>)> {
    let is_finished = state.is_finished.load(Ordering::SeqCst);

    if !is_finished {
        let error_body = ErrorResponse {
            error: "The current job is not yet finished.".to_string(),
        };
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_body)));
    }

    let camera = match state.camera.read() {
        Ok(camera_lock) => Arc::clone(&*camera_lock),
        Err(_) => {
            let error_body = Json(ErrorResponse {
                error: "The global camera lock hs suffered a poisoning".to_string(),
            });
            return Err((StatusCode::INTERNAL_SERVER_ERROR, error_body));
        }
    };

    let world = match state.world.read() {
        Ok(world_lock) => Arc::clone(&*world_lock),
        Err(_) => {
            let error_body = Json(ErrorResponse {
                error: "The global camera lock hs suffered a poisoning".to_string(),
            });
            return Err((StatusCode::INTERNAL_SERVER_ERROR, error_body));
        }
    };

    let fb_worker = Arc::clone(&state.framebuffer);
    let tx_worker = state.tx_stream.clone();
    let is_finished_worker = state.is_finished.clone();

    let tracer = Arc::new(PathTracer {
        max_depth: payload.max_depth.unwrap_or(10),
    });

    tokio::task::spawn_blocking(move || {
        println!("¡Motor de renderizado incializado!");
        is_finished_worker.store(false, std::sync::atomic::Ordering::SeqCst);
        render_scene(
            camera,
            tracer,
            fb_worker,
            tx_worker,
            payload.tile_size.unwrap_or(128),
            3,
            &*world,
        );
        is_finished_worker.store(true, std::sync::atomic::Ordering::SeqCst);
    });

    let message = SuccessMesage {
        message: "job created successfully.".to_string(),
    };

    Ok((StatusCode::CREATED, Json(message)))
}
