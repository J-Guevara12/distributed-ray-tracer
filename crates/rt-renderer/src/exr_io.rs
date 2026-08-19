//! Lectura y escritura de imágenes HDR en radiancia lineal.
//!
//! `save_png` pasa por `to_srgb8`: aplica exposición, tone map y transfer, y
//! cuantiza a 8 bits. Sirve para mirar, no para comparar — el tone map comprime
//! los highlights, que es justo donde vive el ruido que interesa medir.
//!
//! El módulo se llama `exr_io` y no `exr` para no ensombrecer al crate del mismo
//! nombre en las rutas `use`.

use std::path::Path;

use rt_core::Vec3;

pub type Result<T> = std::result::Result<T, ExrError>;

#[derive(Debug)]
pub enum ExrError {
    Codec(exr::error::Error),
    Mismatch(String),
}

impl std::fmt::Display for ExrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExrError::Codec(inner) => write!(f, "exr: {inner}"),
            ExrError::Mismatch(what) => write!(f, "{what}"),
        }
    }
}

impl std::error::Error for ExrError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExrError::Codec(inner) => Some(inner),
            ExrError::Mismatch(_) => None,
        }
    }
}

impl From<exr::error::Error> for ExrError {
    fn from(inner: exr::error::Error) -> Self {
        ExrError::Codec(inner)
    }
}

pub struct HdrImage {
    pub pixels: Vec<Vec3>,
    pub width: u32,
    pub height: u32,
}

impl HdrImage {
    pub fn len(&self) -> usize {
        self.pixels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pixels.is_empty()
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

pub fn write(path: impl AsRef<Path>, pixels: &[Vec3], width: u32, height: u32) -> Result<()> {
    let expected = width as usize * height as usize;
    if pixels.len() != expected {
        return Err(ExrError::Mismatch(format!(
            "{} pixels {width}x{height} (expected {expected})",
            pixels.len()
        )));
    }

    let stride = width as usize;
    exr::prelude::write_rgb_file(path, stride, height as usize, |x, y| {
        let pixel = pixels[y * stride + x];
        (pixel.x, pixel.y, pixel.z)
    })?;

    Ok(())
}

pub fn read(path: impl AsRef<Path>) -> Result<HdrImage> {
    struct Buffer {
        pixels: Vec<Vec3>,
        width: usize,
    }

    let image = exr::prelude::read_first_rgba_layer_from_file(
        path,
        |resolution, _| Buffer {
            pixels: vec![Vec3::ZERO; resolution.width() * resolution.height()],
            width: resolution.width(),
        },
        |buffer: &mut Buffer, position, (r, g, b, _a): (f32, f32, f32, f32)| {
            let index = position.y() * buffer.width + position.x();
            buffer.pixels[index] = Vec3::new(r, g, b);
        },
    )?;

    let size = image.layer_data.size;
    Ok(HdrImage {
        pixels: image.layer_data.channel_data.pixels.pixels,
        width: size.width() as u32,
        height: size.height() as u32,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct Comparison {
    pub mse: f64,
    pub rmse: f64,
    pub relative_mse: f64,
    pub max_abs: f32,
}

const REL_EPSILON: f64 = 1e-2;

pub fn compare(render: &[Vec3], reference: &[Vec3]) -> Result<Comparison> {
    if render.len() != reference.len() {
        return Err(ExrError::Mismatch(format!(
            "el render tiene {} píxeles y la referencia {}",
            render.len(),
            reference.len()
        )));
    }
    if render.is_empty() {
        return Err(ExrError::Mismatch("imagen vacía".into()));
    }

    let mut sum = 0.0f64;
    let mut relative_sum = 0.0f64;
    let mut max_abs = 0.0f32;

    for (a, b) in render.iter().zip(reference.iter()) {
        for channel in 0..3 {
            let (a, b) = (a[channel], b[channel]);
            let error = (a - b) as f64;
            let squared = error * error;

            sum += squared;
            relative_sum += squared / ((b as f64) * (b as f64) + REL_EPSILON);
            max_abs = max_abs.max(error.abs() as f32);
        }
    }

    let n = (render.len() * 3) as f64;
    let mse = sum / n;

    Ok(Comparison {
        mse,
        rmse: mse.sqrt(),
        relative_mse: relative_sum / n,
        max_abs,
    })
}

pub fn efficiency(mse: f64, seconds: f64) -> f64 {
    if mse <= 0.0 || seconds <= 0.0 {
        return 0.0;
    }
    1.0 / (mse * seconds)
}
