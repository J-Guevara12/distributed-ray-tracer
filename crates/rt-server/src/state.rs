use std::sync::Arc;
use rt_renderer::framebuffer::FrameBuffer;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub framebuffer: Arc<FrameBuffer>,
    pub tx_stream: broadcast::Sender<rt_renderer::tiles::TileResult>
}
