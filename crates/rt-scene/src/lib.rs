use rt_core::{Color, Ray, Point3, Vec3, Interval};
use std::fmt::Debug;

pub mod bvh;
pub mod geometry;
pub mod hittable_list;
pub mod materials;
pub mod mesh;
pub mod textures;
mod utils;

pub use bvh::{Aabb, Bvh};

#[derive(Debug, Clone, Copy)]
pub struct HitRecord<'a> {
    pub p: Point3,
    pub normal: Vec3,
    pub t: f32,
    pub front_face: bool,
    pub material: &'a dyn Material,
    /// Coordenadas de textura de la superficie en el punto de impacto.
    pub u: f32,
    pub v: f32,
    /// Tangente geométrica (dirección de u creciente). ZERO si la geometría no la define.
    pub tangent: Vec3,
}

impl<'a> HitRecord<'a> {
    pub fn new(ray: &Ray, t: f32, outward_normal: Vec3, p: Point3, material: &'a dyn Material) -> Self {
        Self::with_uv(ray, t, outward_normal, p, material, 0.0, 0.0, Vec3::ZERO)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_uv(
        ray: &Ray,
        t: f32,
        outward_normal: Vec3,
        p: Point3,
        material: &'a dyn Material,
        u: f32,
        v: f32,
        tangent: Vec3,
    ) -> Self {
        // Determinar si el rayo viene de afuera o de adentro del objeto
        let front_face = ray.direction.dot(outward_normal) < 0.0;
        let normal = if front_face { outward_normal } else { -outward_normal };

        Self { p, normal, t, front_face, material, u, v, tangent }
    }
}

pub trait Hittable: Send + Sync {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>>;

    fn bounding_box(&self) -> Aabb;
}

/// Resultado de la dispersión de un rayo en una superficie.
#[derive(Debug, Clone, Copy)]
pub enum ScatterResult {
    /// Rebote especular (delta): dirección determinista, sin pdf asociada
    /// (espejos, vidrio). La atenuación se aplica directamente.
    Specular { attenuation: Color, scattered: Ray },
    /// Rebote difuso/glossy: la dirección fue muestreada según la pdf del
    /// material. La contribución se evalúa con `bsdf` / `scattering_pdf`,
    /// lo que permite combinarla con muestreo de luces (NEE/MIS).
    Diffuse { scattered: Ray },
}

pub trait Material: Send + Sync + std::fmt::Debug {
    fn scatter(&self, ray_in: &Ray, rec: &HitRecord, rng: &mut fastrand::Rng) -> Option<ScatterResult>;

    /// pdf (en ángulo sólido) con la que `scatter` muestrea `direction`.
    /// Solo es relevante para materiales que devuelven `Diffuse`.
    fn scattering_pdf(&self, _ray_in: &Ray, _rec: &HitRecord, _direction: Vec3) -> f32 {
        0.0
    }

    /// BRDF × cosθ evaluada para una dirección arbitraria de salida.
    /// Solo es relevante para materiales que devuelven `Diffuse`.
    fn bsdf(&self, _ray_in: &Ray, _rec: &HitRecord, _direction: Vec3) -> Color {
        Color::ZERO
    }

    /// Radiancia emitida por la superficie en el punto de impacto.
    fn emitted(&self, _rec: &HitRecord) -> Color {
        Color::ZERO
    }
}

#[cfg(test)]
mod tests;
