use image::ImageError;
use std::{
    fs::File,
    io::BufWriter,
    path::Path,
    sync::{Arc, RwLock},
};

use crate::{post::PostProcess, tiles::Tile};

/// Buffer de acumulación HDR: guarda radiancia lineal en f32 por canal.
/// La conversión a 8 bits (tone mapping + gamma) ocurre solo al exportar.
pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,
    stride: usize,

    data: Arc<RwLock<Vec<f32>>>,
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32, stride: usize) -> Self {
        let size = (width * height) as usize * stride;
        let data = vec![0f32; size];

        Self {
            width,
            height,
            stride,
            data: Arc::new(RwLock::new(data)),
        }
    }

    /// Escribe la radiancia lineal de un tile en la posición correcta del buffer.
    pub fn write_tile_radiance(&self, tile: &Tile, radiance: &[f32]) {
        let stride = self.stride;
        let mut data = self.data.write().unwrap();

        for local_y in 0..tile.height {
            let y = tile.y + local_y;
            let size = tile.width as usize * stride;
            let offset_read = (local_y * tile.width) as usize * stride;
            let offset_write = (y * self.width + tile.x) as usize * stride;

            data[offset_write..offset_write + size]
                .copy_from_slice(&radiance[offset_read..offset_read + size]);
        }
    }

    /// Devuelve una copia snapshot tone-mapeada a 8 bits (útil para previews parciales).
    pub fn get_snapshot(&self, post: &PostProcess) -> Vec<u8> {
        let data = self.data.read().unwrap();

        Self::to_rgb8(&data, post)
    }

    /// Copia cruda de la radiancia lineal acumulada.
    pub fn get_radiance_snapshot(&self) -> Vec<f32> {
        self.data.read().unwrap().clone()
    }

    fn to_rgb8(data: &[f32], post: &PostProcess) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());

        for chunk in data.chunks_exact(3) {
            let color = rt_core::Color::new(chunk[0], chunk[1], chunk[2]);
            out.extend_from_slice(&post.to_rgb8(color));
        }

        out
    }

    pub fn save_png<P: AsRef<Path>>(&self, path: P, post: &PostProcess) -> Result<(), ImageError> {
        let data = self.data.read().unwrap();
        let expected_size = (self.width * self.height * 3) as usize;
        let pixels = Self::to_rgb8(&data[..expected_size], post);

        let file = File::create(path).unwrap();

        let w = &mut BufWriter::new(file);
        let encoder = image::codecs::png::PngEncoder::new(w);

        image::ImageEncoder::write_image(
            encoder,
            &pixels,
            self.width,
            self.height,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();

        Ok(())
    }
}
