use fastrand::Rng;
use rayon::prelude::*;
use rt_scene::{Scene, TraversalStats};
use std::sync::Arc;
use std::time::Instant;
use tracing::{Level, span};

use rt_core::{Color, Vec4};

use crate::{
    camera::Camera, framebuffer::FrameBuffer, stats::{RayStats, RenderStats}, tiles::{TileGenerator, TileResult}, tracers::{RayContext, RayTracer},
};

fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn render_scene<T: RayTracer>(
    camera: Arc<Camera>,
    tracer: Arc<T>,
    framebuffer: Arc<FrameBuffer>,
    on_tile: &(impl Fn(&TileResult) + Send + Sync),
    tile_size: u32,
    scene: &Scene,
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

    let width = camera.width;
    let tiles: Vec<_> = generator.collect();

    let per_tile: Vec<(RayStats, f64)> = tiles
        .par_iter()
        .map(|tile| {
            let mut ctx = RayContext {
                rng: Rng::with_seed(0),
                stats: RayStats::default(),
            };
            let mut pixels: Vec<Vec4> = Vec::with_capacity((tile.width * tile.height) as usize);

            let started = Instant::now();

            for local_y in 0..tile.height {
                for local_x in 0..tile.width {
                    let x = tile.x + local_x;
                    let y = tile.y + local_y;

                    let mut color_accumulator = Color::new(0.0, 0.0, 0.0);

                    for sample in 0..camera.samples_per_pixel {
                        let index = ((y as u64 * width as u64 + x as u64) << 32) | sample as u64;
                        ctx.rng = Rng::with_seed(splitmix64(index));

                        let ray = camera.get_ray(x, y, sample, &mut ctx.rng);
                        let color = tracer.trace_ray(ray, scene, &mut ctx);

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

            (ctx.stats, elapsed_ms)
        })
        .collect();

    let mut traversal = TraversalStats::default();
    for (stats, _) in &per_tile {
        traversal.merge(&stats.traversal);
    }

    RenderStats {
        rays: per_tile.iter().map(|(stats, _)| stats.rays).sum(),
        traversal,
        tile_ms: per_tile.iter().map(|(_, ms)| *ms).collect(),
    }
}
