use std::{fs::File, sync::Arc, time::Instant};

use rt_core::{Color, background::Background, dto::ScenePayload};
use rt_renderer::{
    camera::{Camera, CameraConfig},
    framebuffer::FrameBuffer,
    render::render_scene,
    tracers::PathTracer,
};
use rt_scene::{bvh::BvhNode, hittable_list::HittableList};
use tokio::sync::broadcast;

pub fn main() {
    println!("Inicializando render local");
    let path_scene = "scenes/spheres_scene.json";
    let path_camera = "scenes/spheres_camera.json";

    let file_scene = File::open(path_scene).expect("Error abriendo el archivo de la escena");
    let scene_payload: ScenePayload = serde_json::from_reader(file_scene).unwrap();

    let file_camera = File::open(path_camera).expect("Error abriendo el archivo de la cámara");
    let camera_config: CameraConfig = serde_json::from_reader(file_camera).unwrap();

    let hittable_list = HittableList::from(&scene_payload);
    let world = BvhNode::new(hittable_list.objects);
    let camera = Camera::new(camera_config);

    let framebuffer = Arc::new(FrameBuffer::new(camera.width, camera.height, 3));
    let (tx_stream, _) = broadcast::channel(100);

    let ray_tracer = PathTracer::new(15);
    let instant = Instant::now();
    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));

    render_scene(
        Arc::new(camera),
        Arc::new(ray_tracer),
        Arc::clone(&framebuffer),
        tx_stream,
        128,
        3,
        &world,
        &background,
    );
    println!("Procesado en {} ms", instant.elapsed().as_millis());
    framebuffer.save_png("result.png").unwrap();
}
