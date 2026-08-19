use rt_core::{Interval, Ray};

use crate::{Aabb, HitRecord, Hittable, primitive::Primitive};

/// Barrido lineal sobre todas las primitivas. Es la referencia contra la que se
/// valida cualquier estructura de aceleración: un BVH que no devuelva
/// exactamente esto para todo rayo dejó de ser una optimización transparente.
pub struct LinearScan {
    primitives: Vec<Primitive>,
    bounds: Aabb,
}

impl LinearScan {
    pub fn new(primitives: Vec<Primitive>) -> Self {
        let bounds = primitives
            .iter()
            .map(|p| p.bounding_box())
            .reduce(Aabb::surrounding_box)
            .unwrap_or_default();

        Self { primitives, bounds }
    }
}

impl Hittable for LinearScan {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord> {
        let mut closest = ray_t.max;
        let mut best = None;

        for primitive in &self.primitives {
            if let Some(rec) = primitive.hit(ray, Interval::new(ray_t.min, closest)) {
                closest = rec.t;
                best = Some(rec);
            }
        }

        best
    }

    fn bounding_box(&self) -> Aabb {
        self.bounds
    }
}
