use std::{fs::File, sync::Arc, time::Instant};

use rt_core::dto::ScenePayload;
use rt_renderer::{camera::{Camera, CameraConfig}, framebuffer::FrameBuffer, render::render_scene, tracers::PathTracer};
use rt_scene::hittable_list::HittableList;
use tokio::sync::broadcast;

pub fn main() {
    println!("Inicializando render local");
    let path_scene = "scenes/spheres_scene.json";
    let path_camera = "scenes/spheres_camera.json";

    let file_scene = File::open(path_scene).expect("Error abriendo el archivo de la escena");
    let scene_payload: ScenePayload = serde_json::from_reader(file_scene).unwrap();

    let file_camera = File::open(path_camera).expect("Error abriendo el archivo de la cámara");
    let camera_config: CameraConfig = serde_json::from_reader(file_camera).unwrap();

    let world = HittableList::from(&scene_payload);
    let camera = Camera::new(camera_config);

    let framebuffer = Arc::new(FrameBuffer::new(camera.width, camera.height, 3));
    let (tx_stream, _) = broadcast::channel(100);

    let ray_tracer = PathTracer::new(15);
    let instant = Instant::now();
    render_scene(Arc::new(camera), Arc::new(ray_tracer), Arc::clone(&framebuffer), tx_stream, 128, 3, &world);
    println!("Procesado en {} ms", instant.elapsed().as_millis());
    framebuffer.save_png("result.png").unwrap();
}
