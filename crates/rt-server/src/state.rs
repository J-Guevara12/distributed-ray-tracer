use rt_core::{Job, dto::ScenePayload};
use rt_renderer::{camera::Camera, framebuffer::FrameBuffer, post::PostProcess};
use rt_scene::{Bvh, Hittable};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, AtomicUsize},
};
use tokio::sync::{broadcast, mpsc};

#[derive(Clone)]
pub struct AppState {
    pub framebuffer: Arc<FrameBuffer>,
    pub tx_stream: broadcast::Sender<rt_renderer::tiles::TileResult>,
    pub is_finished: Arc<std::sync::atomic::AtomicBool>,
    pub camera: Arc<RwLock<Arc<Camera>>>,
    pub world: Arc<RwLock<Arc<dyn Hittable + Send + Sync>>>,
    pub scene_data: Arc<RwLock<Option<ScenePayload>>>,
    pub post: Arc<RwLock<PostProcess>>,
    pub job_sender: mpsc::Sender<Job>,
    pub active_jobs_counter: Arc<AtomicUsize>,
}

impl AppState {
    pub fn init_default(n_channels: usize, stride: usize, job_sender: mpsc::Sender<Job>) -> Self {
        let camera = Arc::new(Camera::default());
        let framebuffer = Arc::new(FrameBuffer::new(camera.width, camera.height, stride));
        let (tx_stream, _) = broadcast::channel(n_channels);
        let is_finished = Arc::new(AtomicBool::new(true));
        let active_jobs_counter = Arc::new(AtomicUsize::new(0));

        let camera = Arc::new(RwLock::new(camera));
        let scene_data = ScenePayload::default();
        let world = Arc::new(RwLock::new(
            Arc::new(Bvh::from(&scene_data)) as Arc<dyn Hittable + Send + Sync>,
        ));
        let scene_data = Arc::new(RwLock::new(Some(scene_data)));
        let post = Arc::new(RwLock::new(PostProcess::default()));

        Self {
            framebuffer,
            tx_stream,
            is_finished,
            camera,
            world,
            scene_data,
            post,
            job_sender,
            active_jobs_counter,
        }
    }
}
