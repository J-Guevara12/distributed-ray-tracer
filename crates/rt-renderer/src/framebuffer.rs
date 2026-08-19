use image::ImageError;
use std::{ fs::File, io::BufWriter, path::Path, sync::{ Arc } };
use parking_lot::RwLock;

use crate::exr_io;
use crate::tiles::TileResult;
use rt_core::{Vec4, display::{DisplayParams, resolve, to_srgb8}};

pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,

    data: Arc<RwLock<Vec<Vec4>>>,
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        let data = vec![Vec4::default(); size];

        Self {
            width,
            height,
            data: Arc::new(RwLock::new(data)),
        }
    }

    /// Escribe los píxeles de un TileResult en la posición matemática correcta del buffer.
    pub fn write_tile(&self, result: &TileResult) {
        let mut data = self.data.write();

        for local_y in 0..result.original_tile.height {
            let y = result.original_tile.y + local_y;
            let size = result.original_tile.width as usize;
            let offset_read = (local_y * result.original_tile.width) as usize;
            let offset_write = (y * self.width + result.original_tile.x) as usize;

            data[offset_write..offset_write + size]
                .copy_from_slice(&result.pixels[offset_read..offset_read + size]);
        }
    }

    /// Devuelve una copia snapshot actual de los bytes (útil para guardar imágenes parciales).
    pub fn get_snapshot(&self) -> Vec<Vec4> {
        let data = self.data.read();

        data.to_vec()
    }

    /// Guarda la radiancia lineal tal cual, sin tone mapping ni cuantización.
    /// Es lo que se usa como referencia para medir error; `save_png` es para ver.
    pub fn save_exr<P: AsRef<Path>>(&self, path: P) -> exr_io::Result<()> {
        let data = self.data.read();
        exr_io::write(path, &resolve(&data), self.width, self.height)
    }

    pub fn save_png<P: AsRef<Path>>(&self, path: P, params: &DisplayParams) -> Result<(), ImageError> {
        let data = self.data.read();
        let expected_size = (self.width * self.height * 3) as usize;

        let file = File::create(path)?;

        let w = &mut BufWriter::new(file);
        let encoder = image::codecs::png::PngEncoder::new(w);

        let pixels = to_srgb8(&resolve(&data), params);

        image::ImageEncoder::write_image(
            encoder,
            &pixels[..expected_size],
            self.width,
            self.height,
            image::ExtendedColorType::Rgb8,
        )?;

        Ok(())
    }
}
