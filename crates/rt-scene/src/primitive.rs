use rt_core::{Interval, Ray};

use crate::{
    Aabb, HitRecord, Hittable,
    geometry::{PlanarShape, Sphere},
};

/// Primitiva por valor, para poder guardarlas contiguas en un `Vec`.
pub enum Primitive {
    Sphere(Sphere),
    Planar(Box<PlanarShape>),
}

impl Hittable for Primitive {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord> {
        match self {
            Primitive::Sphere(s) => s.hit(ray, ray_t),
            Primitive::Planar(p) => p.hit(ray, ray_t),
        }
    }

    fn bounding_box(&self) -> Aabb {
        match self {
            Primitive::Sphere(s) => s.bounding_box(),
            Primitive::Planar(p) => p.bounding_box(),
        }
    }
}

impl Primitive {
    /// Clave de orden del constructor: el mínimo de la caja, no el centroide.
    /// Ver el comentario en `bvh.rs` sobre por qué.
    pub fn sort_key(&self, axis: usize) -> f32 {
        let b = self.bounding_box();
        match axis {
            0 => b.x.min,
            1 => b.y.min,
            _ => b.z.min,
        }
    }
}

impl From<Sphere> for Primitive {
    fn from(value: Sphere) -> Self {
        Primitive::Sphere(value)
    }
}

impl From<PlanarShape> for Primitive {
    fn from(value: PlanarShape) -> Self {
        Primitive::Planar(Box::new(value))
    }
}
