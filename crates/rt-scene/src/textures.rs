use std::fmt;
use std::path::Path;
use std::sync::Arc;

use crate::{Color, Point3, Vec3};

/// Fuente de color evaluable en un punto de una superficie.
/// `u`/`v` son las coordenadas de textura; `p` el punto 3D (para texturas procedurales).
pub trait Texture: Send + Sync + fmt::Debug {
    fn value(&self, u: f32, v: f32, p: Point3) -> Color;
}

// =========================================================================
// Color sólido
// =========================================================================

#[derive(Debug, Clone)]
pub struct SolidColor {
    pub albedo: Color,
}

impl SolidColor {
    pub fn new(albedo: Color) -> Self {
        Self { albedo }
    }
}

impl From<Color> for SolidColor {
    fn from(albedo: Color) -> Self {
        Self::new(albedo)
    }
}

impl Texture for SolidColor {
    fn value(&self, _u: f32, _v: f32, _p: Point3) -> Color {
        self.albedo
    }
}

// =========================================================================
// Tablero de ajedrez (3D, basado en la posición espacial)
// =========================================================================

#[derive(Debug)]
pub struct Checker {
    inv_scale: f32,
    even: Arc<dyn Texture>,
    odd: Arc<dyn Texture>,
}

impl Checker {
    pub fn new(scale: f32, even: Arc<dyn Texture>, odd: Arc<dyn Texture>) -> Self {
        Self {
            inv_scale: 1.0 / scale,
            even,
            odd,
        }
    }

    pub fn from_colors(scale: f32, even: Color, odd: Color) -> Self {
        Self::new(
            scale,
            Arc::new(SolidColor::new(even)),
            Arc::new(SolidColor::new(odd)),
        )
    }
}

impl Texture for Checker {
    fn value(&self, u: f32, v: f32, p: Point3) -> Color {
        let x = (p.x * self.inv_scale).floor() as i64;
        let y = (p.y * self.inv_scale).floor() as i64;
        let z = (p.z * self.inv_scale).floor() as i64;

        if (x + y + z) % 2 == 0 {
            self.even.value(u, v, p)
        } else {
            self.odd.value(u, v, p)
        }
    }
}

// =========================================================================
// Textura de imagen (RGB lineal en f32, muestreo bilineal, wrap repeat)
// =========================================================================

pub struct ImageTexture {
    data: Vec<f32>,
    width: u32,
    height: u32,
}

impl fmt::Debug for ImageTexture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageTexture")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl ImageTexture {
    /// Carga una imagen y la convierte a RGB lineal f32.
    /// Los formatos LDR (PNG/JPEG) se asumen sRGB y se linealizan;
    /// los formatos HDR (.hdr) ya son lineales.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, image::ImageError> {
        let path = path.as_ref();
        let img = image::open(path)?;

        let is_hdr = matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("hdr") | Some("exr")
        );

        let rgb = img.to_rgb32f();
        let (width, height) = (rgb.width(), rgb.height());
        let mut data = rgb.into_raw();

        if !is_hdr {
            // sRGB -> lineal (aprox. gamma 2.2)
            for channel in &mut data {
                *channel = channel.powf(2.2);
            }
        }

        Ok(Self {
            data,
            width,
            height,
        })
    }

    /// Textura sintética desde datos crudos (lineales). Útil para tests.
    pub fn from_raw(data: Vec<f32>, width: u32, height: u32) -> Self {
        assert_eq!(data.len(), (width * height * 3) as usize);
        Self {
            data,
            width,
            height,
        }
    }

    #[inline]
    fn texel(&self, x: u32, y: u32) -> Color {
        let idx = ((y * self.width + x) * 3) as usize;
        Color::new(self.data[idx], self.data[idx + 1], self.data[idx + 2])
    }
}

impl Texture for ImageTexture {
    fn value(&self, u: f32, v: f32, _p: Point3) -> Color {
        if self.width == 0 || self.height == 0 {
            return Color::new(1.0, 0.0, 1.0); // magenta de depuración
        }

        // Wrap repeat + V invertida (convención de imagen: fila 0 arriba)
        let uu = u.rem_euclid(1.0);
        let vv = 1.0 - v.rem_euclid(1.0);

        // Muestreo bilineal
        let fx = uu * self.width as f32 - 0.5;
        let fy = vv * self.height as f32 - 0.5;

        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = fx - x0;
        let ty = fy - y0;

        let wrap = |a: f32, n: u32| -> u32 { (a.rem_euclid(n as f32)) as u32 % n };

        let x0i = wrap(x0, self.width);
        let x1i = (x0i + 1) % self.width;
        let y0i = wrap(y0, self.height);
        let y1i = (y0i + 1) % self.height;

        let c00 = self.texel(x0i, y0i);
        let c10 = self.texel(x1i, y0i);
        let c01 = self.texel(x0i, y1i);
        let c11 = self.texel(x1i, y1i);

        let top = c00 * (1.0 - tx) + c10 * tx;
        let bottom = c01 * (1.0 - tx) + c11 * tx;

        top * (1.0 - ty) + bottom * ty
    }
}

// =========================================================================
// Ruido de Perlin (gradientes trilineales + turbulencia)
// =========================================================================

const POINT_COUNT: usize = 256;

pub struct Perlin {
    ranvec: Vec<Vec3>,
    perm_x: Vec<usize>,
    perm_y: Vec<usize>,
    perm_z: Vec<usize>,
}

impl fmt::Debug for Perlin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Perlin").finish()
    }
}

impl Perlin {
    pub fn new(seed: u64) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed);

        let ranvec = (0..POINT_COUNT)
            .map(|_| {
                Vec3::new(
                    rng.f32() * 2.0 - 1.0,
                    rng.f32() * 2.0 - 1.0,
                    rng.f32() * 2.0 - 1.0,
                )
                .normalize()
            })
            .collect();

        Self {
            ranvec,
            perm_x: Self::generate_perm(&mut rng),
            perm_y: Self::generate_perm(&mut rng),
            perm_z: Self::generate_perm(&mut rng),
        }
    }

    fn generate_perm(rng: &mut fastrand::Rng) -> Vec<usize> {
        let mut perm: Vec<usize> = (0..POINT_COUNT).collect();
        for i in (1..POINT_COUNT).rev() {
            let target = rng.usize(0..=i);
            perm.swap(i, target);
        }
        perm
    }

    /// Ruido con interpolación hermítica de gradientes, en [-1, 1].
    pub fn noise(&self, p: Point3) -> f32 {
        let u = p.x - p.x.floor();
        let v = p.y - p.y.floor();
        let w = p.z - p.z.floor();

        let i = p.x.floor() as i64;
        let j = p.y.floor() as i64;
        let k = p.z.floor() as i64;

        let mut accum = 0.0;

        for di in 0..2i64 {
            for dj in 0..2i64 {
                for dk in 0..2i64 {
                    let grad = self.ranvec[self.perm_x[((i + di) & 255) as usize]
                        ^ self.perm_y[((j + dj) & 255) as usize]
                        ^ self.perm_z[((k + dk) & 255) as usize]];

                    let weight = Vec3::new(u - di as f32, v - dj as f32, w - dk as f32);

                    // Suavizado hermítico
                    let uu = u * u * (3.0 - 2.0 * u);
                    let vv = v * v * (3.0 - 2.0 * v);
                    let ww = w * w * (3.0 - 2.0 * w);

                    let fi = di as f32;
                    let fj = dj as f32;
                    let fk = dk as f32;

                    accum += (fi * uu + (1.0 - fi) * (1.0 - uu))
                        * (fj * vv + (1.0 - fj) * (1.0 - vv))
                        * (fk * ww + (1.0 - fk) * (1.0 - ww))
                        * grad.dot(weight);
                }
            }
        }

        accum
    }

    /// Suma de octavas de ruido (turbulencia), en [0, ~1].
    pub fn turbulence(&self, p: Point3, depth: u32) -> f32 {
        let mut accum = 0.0;
        let mut temp_p = p;
        let mut weight = 1.0;

        for _ in 0..depth {
            accum += weight * self.noise(temp_p);
            weight *= 0.5;
            temp_p *= 2.0;
        }

        accum.abs()
    }
}

/// Estilo de visualización del ruido procedural.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseStyle {
    /// Ruido suave directo (remapeado a [0,1]).
    Smooth,
    /// Turbulencia (suma de octavas).
    Turbulence,
    /// Patrón de mármol: seno desplazado por turbulencia.
    Marble,
}

#[derive(Debug)]
pub struct NoiseTexture {
    perlin: Perlin,
    scale: f32,
    style: NoiseStyle,
    color: Color,
}

impl NoiseTexture {
    pub fn new(scale: f32, style: NoiseStyle, color: Color, seed: u64) -> Self {
        Self {
            perlin: Perlin::new(seed),
            scale,
            style,
            color,
        }
    }
}

impl Texture for NoiseTexture {
    fn value(&self, _u: f32, _v: f32, p: Point3) -> Color {
        let scaled = p * self.scale;

        let intensity = match self.style {
            NoiseStyle::Smooth => 0.5 * (1.0 + self.perlin.noise(scaled)),
            NoiseStyle::Turbulence => self.perlin.turbulence(scaled, 7).min(1.0),
            NoiseStyle::Marble => {
                0.5 * (1.0 + (scaled.z + 10.0 * self.perlin.turbulence(p * 1.0, 7)).sin())
            }
        };

        self.color * intensity
    }
}
