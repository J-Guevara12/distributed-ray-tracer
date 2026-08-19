use std::{fs::File, sync::Arc, time::Instant};

use rt_core::{display::DisplayParams, dto::ScenePayload};
use rt_renderer::{
    camera::{Camera, CameraConfig}, framebuffer::FrameBuffer, render::render_scene, tiles::TileResult, tracers::PathTracer,
};
use rt_scene::{Scene, bvh::Bvh, hittable_list::SceneData};

pub fn main() {
    println!("Inicializando render local");
    let path_scene = "scenes/spheres_scene.json";
    let path_camera = "scenes/spheres_camera.json";

    let file_scene = File::open(path_scene).expect("Error when opening spheres_scene.json file");
    let scene_payload: ScenePayload = serde_json::from_reader(file_scene).expect("Error when parsing spheres_scene.json file");

    let file_camera = File::open(path_camera).expect("Error when opening spheres_scene.json file");
    let camera_config: CameraConfig = serde_json::from_reader(file_camera).expect("Error when parsing spheres_scene.json file");

    let data = SceneData::from(&scene_payload);
    let scene = Scene {
        world: Arc::new(Bvh::build(data.objects)),
        materials: data.materials,
        background: scene_payload.background.clone(),
    };
    let camera = Camera::new(camera_config);

    let framebuffer = Arc::new(FrameBuffer::new(camera.width, camera.height));

    let ray_tracer = PathTracer::new(15);
    let instant = Instant::now();

    let on_tile = |_t: &TileResult| { };

    let _ = render_scene(
        Arc::new(camera),
        Arc::new(ray_tracer),
        Arc::clone(&framebuffer),
        &on_tile,
        128,
        &scene,
    );
    println!("Procesado en {} ms", instant.elapsed().as_millis());
    let display_params = DisplayParams::default();
    framebuffer.save_png("result.png", &display_params).expect("Error when saving result.png");
}
