use rayon::prelude::*;
use rt_scene::Hittable;
use std::sync::Arc;
use std::time::Instant;
use tracing::{Level, span};

use rt_core::{Color, Vec4, background::Background};

use crate::{
    camera::Camera,
    framebuffer::FrameBuffer,
    stats::{RayStats, RenderStats},
    tiles::{TileGenerator, TileResult},
    tracers::RayTracer,
};

pub fn render_scene<T: RayTracer>(
    camera: Arc<Camera>,
    tracer: Arc<T>,
    framebuffer: Arc<FrameBuffer>,
    on_tile: &(impl Fn(&TileResult) + Send + Sync),
    tile_size: u32,
    world: &dyn Hittable,
    background: &Background,
) -> RenderStats {
    let generator = TileGenerator::new(camera.width, camera.height, tile_size);
    let samples_float = camera.samples_per_pixel as f32;

    let render_span = span!(
        Level::INFO,
        "scene_render_total",
        width = camera.width,
        height = camera.height
    );
    let _enter = render_span.enter();

    let tiles: Vec<_> = generator.collect();

    let per_tile: Vec<(u64, f64)> = tiles
        .par_iter()
        .map(|tile| {
            let mut ray_stats = RayStats::default();
            let mut pixels: Vec<Vec4> = Vec::with_capacity((tile.width * tile.height) as usize);

            let started = Instant::now();

            for local_y in 0..tile.height {
                for local_x in 0..tile.width {
                    let x = tile.x + local_x;
                    let y = tile.y + local_y;

                    let mut color_accumulator = Color::new(0.0, 0.0, 0.0);

                    for sample in 0..camera.samples_per_pixel {
                        let ray = camera.get_ray(x, y, sample);

                        let color = tracer.trace_ray(ray, world, background, &mut ray_stats);

                        color_accumulator += color;
                    }
                    let pixel_data = Vec4::new(
                        color_accumulator.x,
                        color_accumulator.y,
                        color_accumulator.z,
                        samples_float,
                    );
                    pixels.push(pixel_data)
                }
            }

            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;

            let result = TileResult {
                pixels,
                original_tile: *tile,
            };

            framebuffer.write_tile(&result);
            on_tile(&result);

            (ray_stats.rays, elapsed_ms)
        })
        .collect();

    RenderStats {
        rays: per_tile.iter().map(|(rays, _)| *rays).sum(),
        tile_ms: per_tile.iter().map(|(_, ms)| *ms).collect(),
    }
}
