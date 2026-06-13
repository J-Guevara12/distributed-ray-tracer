use rayon::prelude::*;
use rt_scene::Hittable;
use std::sync::Arc;
use tracing::{Level, span};

use rt_core::Color;
use tokio::sync::broadcast;

use crate::{
    camera::Camera,
    framebuffer::FrameBuffer,
    post::PostProcess,
    tiles::{TileGenerator, TileResult},
    tracers::RayTracer,
};

pub fn render_scene<T: RayTracer>(
    camera: Arc<Camera>,
    tracer: Arc<T>,
    framebuffer: Arc<FrameBuffer>,
    tx_stream: broadcast::Sender<TileResult>,
    tile_size: u32,
    stride: usize,
    post: PostProcess,
    world: &dyn Hittable,
) {
    let generator = TileGenerator::new(camera.width, camera.height, tile_size);
    let samples_inv = 1.0 / camera.samples_per_pixel as f32;

    let render_span = span!(
        Level::INFO,
        "scene_render_total",
        width = camera.width,
        height = camera.height
    );
    let _enter = render_span.enter();

    let tiles: Vec<_> = generator.collect();

    tiles.par_iter().for_each(|tile| {
        let n_pixels = (tile.width * tile.height) as usize;
        let mut radiance: Vec<f32> = Vec::with_capacity(n_pixels * stride);
        let mut pixels: Vec<u8> = Vec::with_capacity(n_pixels * stride);
        // RNG local al tile: evita el acceso thread-local de fastrand en cada llamada
        let mut rng = fastrand::Rng::new();

        for local_y in 0..tile.height {
            for local_x in 0..tile.width {
                let x = tile.x + local_x;
                let y = tile.y + local_y;

                let mut color_accumulator = Color::new(0.0, 0.0, 0.0);

                for sample in 0..camera.samples_per_pixel {
                    let ray = camera.get_ray(x, y, sample, &mut rng);

                    let color = tracer.trace_ray(ray, world, &mut rng);

                    color_accumulator += color;
                }
                let linear = color_accumulator * samples_inv;

                radiance.extend_from_slice(&[linear.x, linear.y, linear.z]);
                pixels.extend_from_slice(&post.to_rgb8(linear));
            }
        }

        framebuffer.write_tile_radiance(tile, &radiance);

        let result = TileResult {
            tile_id: tile.id,
            pixels,
            original_tile: *tile,
        };

        let _ = tx_stream.send(result);
    })
}
