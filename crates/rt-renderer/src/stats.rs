#[derive(Default, Clone)]
pub struct RayStats {
    pub rays: u64,
}

pub struct RenderStats {
    pub rays: u64,
    pub tile_ms: Vec<f64>,
}
