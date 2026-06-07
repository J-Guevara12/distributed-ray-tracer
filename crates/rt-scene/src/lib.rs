use rt_core::{Ray, Point3, Vec3, Interval};
use std::fmt::Debug;

pub mod geometry;
pub mod hittable_list;
pub mod materials;

#[derive(Debug, Clone, Copy)]
pub struct HitRecord<'a> {
    pub p: Point3,
    pub normal: Vec3,
    pub t: f32,
    pub front_face: bool,
    pub material: &'a dyn Material,
}

impl<'a> HitRecord<'a> {
    pub fn new(ray: &Ray, t: f32, outward_normal: Vec3, p: Point3, material: &'a dyn Material) -> Self {
        // Determinar si el rayo viene de afuera o de adentro del objeto
        let front_face = ray.direction.dot(outward_normal) < 0.0;
        let normal = if front_face { outward_normal } else { -outward_normal };

        Self { p, normal, t, front_face , material }
    }
}

pub trait Hittable: Send + Sync {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>>;
}

pub trait Material: Send + Sync + std::fmt::Debug{
    fn scatter(&self, ray_in: &Ray, rec: &HitRecord) -> Option<(Vec3, Ray)>;
}

#[cfg(test)]
mod tests;
