use std::sync::{Arc, RwLock, atomic::AtomicBool};
use rt_core::{Point3, Vec3};
use rt_renderer::{camera::Camera, framebuffer::FrameBuffer};
use rt_scene::{Hittable, geometry::Sphere, hittable_list::HittableList, materials::{Dielectric, Lambertian, Metal}};
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub framebuffer: Arc<FrameBuffer>,
    pub tx_stream: broadcast::Sender<rt_renderer::tiles::TileResult>,
    pub is_finished: Arc<std::sync::atomic::AtomicBool>,
    pub camera: Arc<RwLock<Arc<Camera>>>,
    pub world: Arc<RwLock<Arc<dyn Hittable + Send + Sync>>>,
}

impl AppState {
    pub fn init_default(n_channels: usize, stride: usize) -> Self {
        let camera = Arc::new(Camera::default());
        let framebuffer = Arc::new(FrameBuffer::new(camera.width, camera.height, stride));
        let (tx_stream, _) = broadcast::channel(n_channels);
        let is_finished = Arc::new(AtomicBool::new(true));
        let mut world = HittableList::new();

        let material_main = Arc::new(Lambertian::new(Vec3::new(1.0, 0.0, 0.0)));
        let material_2 = Arc::new(Metal::new(Vec3::new(1.0, 1.0, 1.0), 0.0));
        let material_3 = Arc::new(Metal::new(Vec3::new(0.0, 0.4, 0.8), 0.6));
        let material_4 = Arc::new(Dielectric::new(1.5));
        let material_5 = Arc::new(Dielectric::new(1.0/1.5));
        let material_ground = Arc::new(Lambertian::new(Vec3::new(0.0, 0.9, 0.2)));


        world.add(Arc::new(Sphere::new(Point3::new(-1.3, 0.0, -1.4), 0.5, material_2)));
        world.add(Arc::new(Sphere::new(Point3::new(1.3, 0.0, -1.8), 0.5, material_3)));
        world.add(Arc::new(Sphere::new(Point3::new(0.0, 0.8, -1.4), 0.5, material_4)));
        world.add(Arc::new(Sphere::new(Point3::new(0.0, 0.8, -1.4), 0.1, material_5)));
        world.add(Arc::new(Sphere::new(Point3::new(0.0, 0.0, -1.0), 0.5, material_main)));
        world.add(Arc::new(Sphere::new(Point3::new(0.0, -100.5, -1.0), 100.0, material_ground)));

        let camera = Arc::new(RwLock::new(camera));
        let world = Arc::new(RwLock::new(Arc::new(world) as Arc<dyn Hittable + Send + Sync>));

        Self { framebuffer, tx_stream, is_finished, camera , world }
    }
}
