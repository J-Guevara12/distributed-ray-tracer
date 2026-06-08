use std::sync::{Arc, atomic::Ordering};

use axum::{Json, extract::State, http::StatusCode};
use rt_core::{Point3, Vec3};
use rt_renderer::{render::render_scene, tracers::PathTracer};
use rt_scene::{geometry::Sphere, hittable_list::HittableList, materials::{Dielectric, Lambertian, Metal}};

use crate::{handlers::ErrorResponse, state::AppState};

#[derive(serde::Serialize)]
pub struct SuccessMesage {
    pub message: String
}

#[derive(serde::Deserialize)]
pub struct CreateJobPayload {
    pub tile_size: Option<u32>,
    pub max_depth: Option<u32>,
}

pub async fn post_render(
    State(state): State<AppState>,
    Json(payload): Json<CreateJobPayload>
) -> Result<(StatusCode, Json<SuccessMesage>),(StatusCode, Json<ErrorResponse>)>{
    let is_finished = state.is_finished.load(Ordering::SeqCst);

    if !is_finished {
        let error_body = ErrorResponse{
            error: "The current job is not yet finished.".to_string()
        };
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_body)))
    }

    let camera;

    match state.camera.read() {
        Ok(camera_lock) => {
            camera = Arc::clone(&*camera_lock)
        },
        Err(_) => {
            let error_body = Json(ErrorResponse{
                error: "The global camera lock hs suffered a poisoning".to_string(),
            });
            return Err((StatusCode::INTERNAL_SERVER_ERROR, error_body));
        },
    }

    let material_main = Arc::new(Lambertian::new(Vec3::new(1.0, 0.0, 0.0)));
    let material_2 = Arc::new(Metal::new(Vec3::new(1.0, 1.0, 1.0), 0.0));
    let material_3 = Arc::new(Metal::new(Vec3::new(0.0, 0.4, 0.8), 0.6));
    let material_4 = Arc::new(Dielectric::new(1.5));
    let material_5 = Arc::new(Dielectric::new(1.0/1.5));
    let material_ground = Arc::new(Lambertian::new(Vec3::new(0.0, 0.9, 0.2)));

    let mut world = HittableList::new();

    world.add(Arc::new(Sphere::new(Point3::new(-1.3, 0.0, -1.4), 0.5, material_2)));
    world.add(Arc::new(Sphere::new(Point3::new(1.3, 0.0, -1.8), 0.5, material_3)));
    world.add(Arc::new(Sphere::new(Point3::new(0.0, 0.8, -1.4), 0.5, material_4)));
    world.add(Arc::new(Sphere::new(Point3::new(0.0, 0.8, -1.4), 0.1, material_5)));
    world.add(Arc::new(Sphere::new(Point3::new(0.0, 0.0, -1.0), 0.5, material_main)));
    world.add(Arc::new(Sphere::new(Point3::new(0.0, -100.5, -1.0), 100.0, material_ground)));

    let fb_worker = Arc::clone(&state.framebuffer);
    let tx_worker = state.tx_stream.clone();
    let is_finished_worker = state.is_finished.clone();

    let tracer = Arc::new(PathTracer{max_depth: payload.max_depth.unwrap_or(10)});

    tokio::task::spawn_blocking(move || {
        println!("¡Motor de renderizado incializado!");
        is_finished_worker.store(false, std::sync::atomic::Ordering::SeqCst);
        render_scene(camera, tracer, fb_worker, tx_worker, payload.tile_size.unwrap_or(128), 3, &world);
        is_finished_worker.store(true, std::sync::atomic::Ordering::SeqCst);
    });

    let message = SuccessMesage{
        message: "job created successfully.".to_string()
    };

    Ok((StatusCode::CREATED, Json(message)))
}

