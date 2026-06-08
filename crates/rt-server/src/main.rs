use axum::{routing::get, Router};
use rt_core::{Point3, Vec3};
use rt_renderer::{camera::{Camera, CameraConfig}, framebuffer::FrameBuffer, render::render_scene, tracers::PathTracer};
use rt_scene::{geometry::Sphere, hittable_list::HittableList, materials::{Dielectric, Lambertian, Metal}};
use tokio::sync::broadcast;
use tower_http::cors::{CorsLayer, Any};
use std::{net::SocketAddr, sync::{Arc, atomic::AtomicBool}, time::Instant};
use tracing_subscriber::fmt::format::FmtSpan;

use crate::state::AppState;
mod state;
mod handlers;

#[tokio::main]
async fn main() {
    let (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stdout());

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_max_level(tracing::Level::INFO)
        .with_span_events(FmtSpan::CLOSE)
        .init();
    let aspect_ratio = 16.0 / 9.0;
    let image_width = 1920;
    let stride = 3;
    let tile_size = 128;

    let camera_config = CameraConfig{
        aspect_ratio,
        image_width,
        fov: 90.0,
        look_from: Point3::new(0.0, 0.0, 0.0),
        look_at: Point3::new(0.0, 0.0, -1.0),
        vup: Point3::new(0.0, 1.0, 0.0),
        samples_per_pixel: 100,
    };
    
    let camera = Arc::new(Camera::new(camera_config));

    let width = camera.width;
    let height = camera.height;

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


    println!("Iniciando rt-server...");
    println!("Resolución de renderizado: {}x{} (Tile Size: {})", width, height, tile_size);

    let framebuffer = Arc::new(FrameBuffer::new(width, height, stride));
    let (tx_stream, _) = broadcast::channel(100);

    let state = AppState {
        framebuffer: Arc::clone(&framebuffer),
        tx_stream: tx_stream.clone(),
        is_finished: Arc::new(AtomicBool::new(false)),
    };

    let camera_worker = Arc::clone(&camera);
    let fb_worker = Arc::clone(&framebuffer);
    let tx_worker = tx_stream.clone();
    let is_finished_worker = state.is_finished.clone();

    let tracer = Arc::new(PathTracer{max_depth: 10});

    tokio::task::spawn_blocking(move || {
        println!("¡Motor de renderizado incializado!");
        let instant = Instant::now(); 
        render_scene(camera_worker, tracer, fb_worker, tx_worker, tile_size, stride, &world);
        is_finished_worker.store(true, std::sync::atomic::Ordering::SeqCst);
        println!("¡Renderizado completado! {} ms", instant.elapsed().as_millis());
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any);

    let app = Router::new()
        .route("/health", get(handlers::health_handler))
        .route("/render/stream", get(handlers::render_stream_handler))
        .with_state(state)
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 1, 1], 3000));
    println!("Servidor de pruebas corriendo en http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests;
