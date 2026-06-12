use image::ImageError;
use std::{
    fs::File,
    io::BufWriter,
    path::Path,
    sync::{Arc, RwLock},
};

use crate::tiles::TileResult;

pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,

    data: Arc<RwLock<Vec<u8>>>,
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32, stride: usize) -> Self {
        let size = (width * height) as usize * stride;
        let data = vec![0u8; size];

        Self {
            width,
            height,
            data: Arc::new(RwLock::new(data)),
        }
    }

    /// Escribe los píxeles de un TileResult en la posición matemática correcta del buffer.
    pub fn write_tile(&self, result: &TileResult, stride: usize) {
        let mut data = self.data.write().unwrap();

        for local_y in 0..result.original_tile.height {
            let y = result.original_tile.y + local_y;
            let size = result.original_tile.width as usize * stride;
            let offset_read = (local_y * result.original_tile.width) as usize * stride;
            let offset_write = (y * self.width + result.original_tile.x) as usize * stride;

            data[offset_write..offset_write + size]
                .copy_from_slice(&result.pixels[offset_read..offset_read + size]);
        }
    }

    /// Devuelve una copia snapshot actual de los bytes (útil para guardar imágenes parciales).
    pub fn get_snapshot(&self) -> Vec<u8> {
        let data = self.data.read().unwrap();

        data.to_vec()
    }

    pub fn save_png<P: AsRef<Path>>(&self, path: P) -> Result<(), ImageError> {
        let data = self.data.read().unwrap();
        let expected_size = (self.width * self.height * 3) as usize;

        let file = File::create(path).unwrap();

        let w = &mut BufWriter::new(file);
        let encoder = image::codecs::png::PngEncoder::new(w);

        image::ImageEncoder::write_image(
            encoder,
            &data[..expected_size],
            self.width,
            self.height,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();

        Ok(())
    }
}
