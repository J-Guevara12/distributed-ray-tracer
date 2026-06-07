use std::sync::Arc;
use tracing::{span, Level};
use rayon::prelude::*;

use rt_core::RayTracer;
use tokio::sync::broadcast;

use crate::{camera::Camera, framebuffer::FrameBuffer, tiles::{TileGenerator, TileResult}};

pub fn render_scene<T: RayTracer> (
    camera: Arc<Camera>,
    tracer: Arc<T>,
    framebuffer: Arc<FrameBuffer>,
    tx_stream: broadcast::Sender<TileResult>,
    tile_size: u32,
    stride: usize
){
    let generator = TileGenerator::new(camera.width, camera.height, tile_size);

    let render_span = span!(Level::INFO, "scene_render_total", width=camera.width, height = camera.height);
    let _enter = render_span.enter();

    let tiles: Vec<_> = generator.collect();

    tiles.par_iter().for_each(|tile| {
        let tile_span = span!(Level::DEBUG, "render_tile", tile_id = tile.id);
        let _tile_enter = tile_span.enter();
        let mut pixels: Vec<u8> = Vec::with_capacity((tile.width * tile.height) as usize * stride);

        for local_y in 0..tile.height {
            for local_x in 0..tile.width {
                let x = tile.x + local_x;
                let y = tile.y + local_y;

                let mut r = 0.0;
                let mut g = 0.0;
                let mut b = 0.0;

                for sample in 0..camera.samples_per_pixel {
                    let ray = camera.get_ray(x, y, sample);

                    let color = tracer.trace_ray(ray);

                    r += color[0] as f32;
                    g += color[1] as f32;
                    b += color[2] as f32;
                }

                pixels.push((r/camera.samples_per_pixel as f32) as u8);
                pixels.push((g/camera.samples_per_pixel as f32) as u8);
                pixels.push((b/camera.samples_per_pixel as f32) as u8);
            }
        }
        let result = TileResult { tile_id: tile.id, pixels, original_tile: *tile };

        framebuffer.write_tile(&result, stride);

        let _ = tx_stream.send(result);
    })
}
