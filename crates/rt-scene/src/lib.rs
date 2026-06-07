use rt_core::{Ray, Point3, Vec3, Interval};
pub mod geometry;
pub mod hittable_list;

#[derive(Debug, Clone, Copy)]
pub struct HitRecord {
    pub p: Point3,
    pub normal: Vec3,
    pub t: f32,
    pub front_face: bool,
}

impl HitRecord {
    pub fn new(ray: &Ray, t: f32, outward_normal: Vec3, p: Point3) -> Self {
        // Determinar si el rayo viene de afuera o de adentro del objeto
        let front_face = ray.direction.dot(outward_normal) < 0.0;
        let normal = if front_face { outward_normal } else { -outward_normal };

        Self { p, normal, t, front_face }
    }
}

pub trait Hittable: Send + Sync {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord>;
}

#[cfg(test)]
mod tests;
