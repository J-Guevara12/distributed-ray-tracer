use std::f32::consts::PI;
use std::sync::Arc;

use rt_core::{Color, Ray, Vec3};

use crate::textures::{SolidColor, Texture};
use crate::utils::*;
use crate::{HitRecord, Material, ScatterResult};

// =========================================================================
// Mapas de superficie: normal mapping + parallax mapping
// =========================================================================

/// Mapas opcionales que perturban la superficie en el punto de impacto.
/// El normal map perturba la normal de shading; el height map desplaza
/// las UV según el ángulo de visión (parallax por offset).
#[derive(Debug, Clone)]
pub struct SurfaceMaps {
    pub normal_map: Option<Arc<dyn Texture>>,
    pub height_map: Option<Arc<dyn Texture>>,
    pub height_scale: f32,
}

impl Default for SurfaceMaps {
    fn default() -> Self {
        Self {
            normal_map: None,
            height_map: None,
            height_scale: 0.05,
        }
    }
}

impl SurfaceMaps {
    pub fn is_empty(&self) -> bool {
        self.normal_map.is_none() && self.height_map.is_none()
    }

    /// Devuelve (normal de shading, u, v) tras aplicar parallax y normal mapping.
    pub fn shade(&self, ray_in: &Ray, rec: &HitRecord) -> (Vec3, f32, f32) {
        if self.is_empty() {
            return (rec.normal, rec.u, rec.v);
        }

        // Base tangencial (T, B, N), ortonormalizada respecto a la normal
        let n = rec.normal;
        let mut t = rec.tangent;
        if t.length_squared() < 1e-12 {
            (t, _) = orthonormal_basis(n);
        }
        t = (t - n * n.dot(t)).normalize();
        let b = n.cross(t);

        let (mut u, mut v) = (rec.u, rec.v);

        // Parallax por offset: desplaza las UV en la dirección de visión proyectada
        if let Some(height_map) = &self.height_map {
            let view = -ray_in.direction.normalize();
            let view_ts = Vec3::new(view.dot(t), view.dot(b), view.dot(n));

            let height = height_map.value(u, v, rec.p).x;
            let z = view_ts.z.max(0.2); // límite para evitar derrapes en ángulos rasantes
            u -= view_ts.x / z * height * self.height_scale;
            v -= view_ts.y / z * height * self.height_scale;
        }

        // Normal mapping: la textura codifica la normal tangencial en [0,1]
        let normal = if let Some(normal_map) = &self.normal_map {
            let sample = normal_map.value(u, v, rec.p) * 2.0 - Color::ONE;
            (t * sample.x + b * sample.y + n * sample.z).normalize()
        } else {
            n
        };

        (normal, u, v)
    }
}

// =========================================================================
// Lambertian (difuso con muestreo coseno)
// =========================================================================

#[derive(Debug)]
pub struct Lambertian {
    pub albedo: Arc<dyn Texture>,
    pub maps: SurfaceMaps,
}

impl Lambertian {
    pub fn new(albedo: Color) -> Self {
        Self::textured(Arc::new(SolidColor::new(albedo)))
    }

    pub fn textured(albedo: Arc<dyn Texture>) -> Self {
        Self {
            albedo,
            maps: SurfaceMaps::default(),
        }
    }

    pub fn with_maps(albedo: Arc<dyn Texture>, maps: SurfaceMaps) -> Self {
        Self { albedo, maps }
    }
}

impl Material for Lambertian {
    fn scatter(&self, ray_in: &Ray, rec: &HitRecord, rng: &mut fastrand::Rng) -> Option<ScatterResult> {
        let (normal, _, _) = self.maps.shade(ray_in, rec);

        // Muestreo coseno: normal + dirección uniforme en la esfera unitaria
        let mut scatter_direction = normal + random_unit_vector(rng);

        if is_near_zero(&scatter_direction) {
            scatter_direction = normal
        }

        let scattered = Ray::new_at_time(rec.p, scatter_direction, ray_in.time);

        Some(ScatterResult::Diffuse { scattered })
    }

    fn scattering_pdf(&self, ray_in: &Ray, rec: &HitRecord, direction: Vec3) -> f32 {
        let (normal, _, _) = self.maps.shade(ray_in, rec);
        let cosine = normal.dot(direction.normalize());

        (cosine / PI).max(0.0)
    }

    fn bsdf(&self, ray_in: &Ray, rec: &HitRecord, direction: Vec3) -> Color {
        let (normal, u, v) = self.maps.shade(ray_in, rec);
        let cosine = normal.dot(direction.normalize()).max(0.0);

        self.albedo.value(u, v, rec.p) * (cosine / PI)
    }
}

// =========================================================================
// Metal (reflexión especular con fuzz)
// =========================================================================

#[derive(Debug)]
pub struct Metal {
    pub albedo: Arc<dyn Texture>,
    pub fuzz: f32,
    pub maps: SurfaceMaps,
}

impl Metal {
    pub fn new(albedo: Color, fuzz: f32) -> Self {
        Self::textured(Arc::new(SolidColor::new(albedo)), fuzz)
    }

    pub fn textured(albedo: Arc<dyn Texture>, fuzz: f32) -> Self {
        Self {
            albedo,
            fuzz,
            maps: SurfaceMaps::default(),
        }
    }

    pub fn with_maps(albedo: Arc<dyn Texture>, fuzz: f32, maps: SurfaceMaps) -> Self {
        Self { albedo, fuzz, maps }
    }
}

impl Material for Metal {
    fn scatter(&self, ray_in: &Ray, rec: &HitRecord, rng: &mut fastrand::Rng) -> Option<ScatterResult> {
        let (normal, u, v) = self.maps.shade(ray_in, rec);

        let reflected = reflect(ray_in.direction, normal);

        let scatter_direction = reflected + self.fuzz * random_unit_vector(rng);

        if scatter_direction.dot(normal) > 0.0 {
            Some(ScatterResult::Specular {
                attenuation: self.albedo.value(u, v, rec.p),
                scattered: Ray::new_at_time(rec.p, scatter_direction, ray_in.time),
            })
        } else {
            None // Si el fuzz empujó el rayo hacia adentro de la geometría, la luz se absorbe
        }
    }
}

// =========================================================================
// Dielectric (refracción con Schlick)
// =========================================================================

#[derive(Debug)]
pub struct Dielectric {
    pub refraction_index: f32, // Índice de refracción (ej: 1.5)
}

impl Dielectric {
    pub fn new(refraction_index: f32) -> Self {
        Self { refraction_index }
    }
}

impl Material for Dielectric {
    fn scatter(&self, ray_in: &Ray, rec: &HitRecord, rng: &mut fastrand::Rng) -> Option<ScatterResult> {
        let attenuation = Color::ONE;

        let ri = if rec.front_face {
            1.0 / self.refraction_index
        } else {
            self.refraction_index
        };

        let cos_theta = (-ray_in.direction).dot(rec.normal).min(1.0);
        let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

        let cannot_refract = ri * sin_theta > 1.0;

        let scatter_direction = if cannot_refract || reflectance(cos_theta, ri) > rng.f32() {
            // Caso A: Reflexión interna total o probabilidad alta según Schlick -> El rayo rebota como espejo
            reflect(ray_in.direction, rec.normal)
        } else {
            // Caso B: El rayo se refracta y atraviesa el material de forma física
            refract(ray_in.direction, rec.normal, ri)
        };

        Some(ScatterResult::Specular {
            attenuation,
            scattered: Ray::new_at_time(rec.p, scatter_direction, ray_in.time),
        })
    }
}

// =========================================================================
// DiffuseLight (material emisivo)
// =========================================================================

#[derive(Debug)]
pub struct DiffuseLight {
    pub emit: Arc<dyn Texture>,
    pub intensity: f32,
}

impl DiffuseLight {
    pub fn new(color: Color, intensity: f32) -> Self {
        Self {
            emit: Arc::new(SolidColor::new(color)),
            intensity,
        }
    }

    pub fn textured(emit: Arc<dyn Texture>, intensity: f32) -> Self {
        Self { emit, intensity }
    }
}

impl Material for DiffuseLight {
    fn scatter(&self, _ray_in: &Ray, _rec: &HitRecord, _rng: &mut fastrand::Rng) -> Option<ScatterResult> {
        None
    }

    fn emitted(&self, rec: &HitRecord) -> Color {
        // Emisión solo por la cara frontal (luces de una cara)
        if rec.front_face {
            self.emit.value(rec.u, rec.v, rec.p) * self.intensity
        } else {
            Color::ZERO
        }
    }
}

// =========================================================================
// Isotropic (función de fase de medios participativos)
// =========================================================================

#[derive(Debug)]
pub struct Isotropic {
    pub albedo: Arc<dyn Texture>,
}

impl Isotropic {
    pub fn new(albedo: Color) -> Self {
        Self {
            albedo: Arc::new(SolidColor::new(albedo)),
        }
    }

    pub fn textured(albedo: Arc<dyn Texture>) -> Self {
        Self { albedo }
    }
}

const INV_4PI: f32 = 1.0 / (4.0 * PI);

impl Material for Isotropic {
    fn scatter(&self, ray_in: &Ray, rec: &HitRecord, rng: &mut fastrand::Rng) -> Option<ScatterResult> {
        let scattered = Ray::new_at_time(rec.p, random_unit_vector(rng), ray_in.time);

        Some(ScatterResult::Diffuse { scattered })
    }

    fn scattering_pdf(&self, _ray_in: &Ray, _rec: &HitRecord, _direction: Vec3) -> f32 {
        INV_4PI
    }

    fn bsdf(&self, _ray_in: &Ray, rec: &HitRecord, _direction: Vec3) -> Color {
        // Función de fase uniforme: sin término coseno
        self.albedo.value(rec.u, rec.v, rec.p) * INV_4PI
    }
}

// =========================================================================
// GGX (microfacetas Cook-Torrance, solo reflexión)
// =========================================================================

#[derive(Debug)]
pub struct Ggx {
    pub albedo: Arc<dyn Texture>,
    /// Rugosidad perceptual en [0,1]; α = roughness².
    pub roughness: f32,
    pub maps: SurfaceMaps,
}

impl Ggx {
    pub fn new(albedo: Color, roughness: f32) -> Self {
        Self::textured(Arc::new(SolidColor::new(albedo)), roughness)
    }

    pub fn textured(albedo: Arc<dyn Texture>, roughness: f32) -> Self {
        Self {
            albedo,
            roughness,
            maps: SurfaceMaps::default(),
        }
    }

    pub fn with_maps(albedo: Arc<dyn Texture>, roughness: f32, maps: SurfaceMaps) -> Self {
        Self {
            albedo,
            roughness,
            maps,
        }
    }

    #[inline]
    fn alpha(&self) -> f32 {
        (self.roughness * self.roughness).max(1e-3)
    }

    /// Distribución de normales GGX (Trowbridge-Reitz).
    fn ndf(alpha: f32, cos_h: f32) -> f32 {
        let a2 = alpha * alpha;
        let denom = cos_h * cos_h * (a2 - 1.0) + 1.0;
        a2 / (PI * denom * denom)
    }

    /// Término geométrico de Smith (forma separable, GGX).
    fn smith_g(alpha: f32, cos_v: f32, cos_l: f32) -> f32 {
        let g1 = |cos: f32| {
            let a2 = alpha * alpha;
            2.0 * cos / (cos + (a2 + (1.0 - a2) * cos * cos).sqrt())
        };
        g1(cos_v) * g1(cos_l)
    }

    /// Fresnel de Schlick con F0 cromático.
    fn fresnel(f0: Color, cos: f32) -> Color {
        f0 + (Color::ONE - f0) * (1.0 - cos).clamp(0.0, 1.0).powi(5)
    }

    /// Muestrea un half-vector según la NDF GGX alrededor de `normal`.
    fn sample_half_vector(&self, normal: Vec3, rng: &mut fastrand::Rng) -> Vec3 {
        let alpha = self.alpha();
        let r1 = rng.f32();
        let r2 = rng.f32();

        let cos_theta = ((1.0 - r1) / (r1 * (alpha * alpha - 1.0) + 1.0)).sqrt();
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let phi = 2.0 * PI * r2;

        let (t, b) = orthonormal_basis(normal);
        (t * (sin_theta * phi.cos()) + b * (sin_theta * phi.sin()) + normal * cos_theta).normalize()
    }
}

impl Material for Ggx {
    fn scatter(&self, ray_in: &Ray, rec: &HitRecord, rng: &mut fastrand::Rng) -> Option<ScatterResult> {
        let (normal, _, _) = self.maps.shade(ray_in, rec);

        let half = self.sample_half_vector(normal, rng);
        let direction = reflect(ray_in.direction, half);

        if direction.dot(normal) <= 0.0 {
            return None; // Muestra bajo el horizonte: se absorbe
        }

        Some(ScatterResult::Diffuse {
            scattered: Ray::new_at_time(rec.p, direction, ray_in.time),
        })
    }

    fn scattering_pdf(&self, ray_in: &Ray, rec: &HitRecord, direction: Vec3) -> f32 {
        let (normal, _, _) = self.maps.shade(ray_in, rec);

        let view = -ray_in.direction.normalize();
        let light = direction.normalize();

        if light.dot(normal) <= 0.0 || view.dot(normal) <= 0.0 {
            return 0.0;
        }

        let half = (view + light).normalize();
        let cos_h = half.dot(normal).max(0.0);
        let v_dot_h = view.dot(half).max(1e-6);

        // pdf de l: pdf(h) = D·cosθh, con jacobiano del reflejo 1/(4·v·h)
        Self::ndf(self.alpha(), cos_h) * cos_h / (4.0 * v_dot_h)
    }

    fn bsdf(&self, ray_in: &Ray, rec: &HitRecord, direction: Vec3) -> Color {
        let (normal, u, v) = self.maps.shade(ray_in, rec);

        let view = -ray_in.direction.normalize();
        let light = direction.normalize();

        let cos_v = view.dot(normal);
        let cos_l = light.dot(normal);

        if cos_v <= 0.0 || cos_l <= 0.0 {
            return Color::ZERO;
        }

        let half = (view + light).normalize();
        let cos_h = half.dot(normal).max(0.0);
        let v_dot_h = view.dot(half).max(0.0);

        let alpha = self.alpha();
        let f0 = self.albedo.value(u, v, rec.p);

        let d = Self::ndf(alpha, cos_h);
        let g = Self::smith_g(alpha, cos_v, cos_l);
        let f = Self::fresnel(f0, v_dot_h);

        // BRDF de Cook-Torrance multiplicada por cosθl:
        // D·G·F / (4·cosθv·cosθl) · cosθl = D·G·F / (4·cosθv)
        f * (d * g / (4.0 * cos_v))
    }
}
