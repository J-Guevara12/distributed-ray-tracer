use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{Json, extract::State, http::StatusCode};
use rt_renderer::{render::render_scene, tiles::{TilePatch, TileResult}, tracers::PathTracer};

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

struct JobGuard(Arc<AtomicBool>);

impl Drop for JobGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

pub async fn post_render(
    State(state): State<AppState>,
    Json(payload): Json<CreateJobPayload>,
) -> Result<(StatusCode, Json<SuccessMesage>), (StatusCode, Json<ErrorResponse>)> {
    let camera = Arc::clone(&state.camera.read());
    let world = Arc::clone(&state.world.read());

    let background = match state.scene_data.read().as_ref() {
        Some(payload) => payload.background.clone(),
        None => {
            let error_body = Json(ErrorResponse {
                error: "No scene has been loaded yet.".to_string(),
            });
            return Err((StatusCode::CONFLICT, error_body));
        }
    };

    if state
        .is_finished
        .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        let error_body = Json(ErrorResponse {
            error: "A render job is already in progress.".to_string(),
        });
        return Err((StatusCode::CONFLICT, error_body));
    }

    let fb_worker = Arc::clone(&state.framebuffer);
    let tx_worker = state.tx_stream.clone();
    let is_finished_worker = Arc::clone(&state.is_finished);

    let tracer = Arc::new(PathTracer {
        max_depth: payload.max_depth.unwrap_or(10),
    });
    let display_params = Arc::clone(&state.display_params);

    let on_tile = move |t: &TileResult| {
        // Sin suscriptores no vale la pena pagar el tone mapping del tile.
        if tx_worker.receiver_count() == 0 {
            return;
        }
        let params = *display_params.read();
        let patch = TilePatch::from_tile_result(t, &params);
        let _ = tx_worker.send(patch);
    };

    tokio::task::spawn_blocking(move || {
        let _guard = JobGuard(is_finished_worker);

        println!("¡Motor de renderizado incializado!");
        let _ = render_scene(
            camera,
            tracer,
            fb_worker,
            &on_tile,
            payload.tile_size.unwrap_or(128),
            &*world,
            &background,
        );
    });

    let message = SuccessMesage {
        message: "job created successfully.".to_string(),
    };

    Ok((StatusCode::CREATED, Json(message)))
}
