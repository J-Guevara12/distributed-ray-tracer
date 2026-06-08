use std::sync::{Arc, RwLock, atomic::AtomicBool};
use rt_renderer::{camera::Camera, framebuffer::FrameBuffer};
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub framebuffer: Arc<FrameBuffer>,
    pub tx_stream: broadcast::Sender<rt_renderer::tiles::TileResult>,
    pub is_finished: Arc<std::sync::atomic::AtomicBool>,
    pub camera: Arc<RwLock<Arc<Camera>>>
}

impl AppState {
    pub fn init_default(n_channels: usize, stride: usize) -> Self {
        let camera = Arc::new(Camera::default());
        let framebuffer = Arc::new(FrameBuffer::new(camera.width, camera.height, stride));
        let (tx_stream, _) = broadcast::channel(n_channels);
        let is_finished = Arc::new(AtomicBool::new(true));

        let camera = Arc::new(RwLock::new(camera));

        Self { framebuffer, tx_stream, is_finished, camera }
    }
}
