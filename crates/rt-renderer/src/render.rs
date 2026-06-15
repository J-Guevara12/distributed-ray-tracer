use rayon::prelude::*;
use rt_scene::Hittable;
use std::sync::Arc;
use tracing::{Level, span};

use rt_core::{Color, background::Background};
use tokio::sync::broadcast;

use crate::{
    camera::Camera,
    framebuffer::FrameBuffer,
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
    world: &dyn Hittable,
    background: &Background,
) {
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

    tiles.par_iter().for_each(|tile| {
        let mut pixels: Vec<u8> = Vec::with_capacity((tile.width * tile.height) as usize * stride);

        for local_y in 0..tile.height {
            for local_x in 0..tile.width {
                let x = tile.x + local_x;
                let y = tile.y + local_y;

                let mut color_accumulator = Color::new(0.0, 0.0, 0.0);

                for sample in 0..camera.samples_per_pixel {
                    let ray = camera.get_ray(x, y, sample);

                    let color = tracer.trace_ray(ray, world, background);

                    color_accumulator += color;
                }
                let gamma_corrected = 255.9 * (color_accumulator / samples_float).sqrt();

                pixels.push(gamma_corrected[0] as u8);
                pixels.push(gamma_corrected[1] as u8);
                pixels.push(gamma_corrected[2] as u8);
            }
        }
        let result = TileResult {
            tile_id: tile.id,
            pixels,
            original_tile: *tile,
        };

        framebuffer.write_tile(&result, stride);

        let _ = tx_stream.send(result);
    })
}
