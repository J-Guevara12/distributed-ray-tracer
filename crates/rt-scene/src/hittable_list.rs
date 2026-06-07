use crate::*;

use std::sync::Arc;
use crate::Hittable;

#[derive(Default)]
pub struct HittableList {
    pub objects: Vec<Arc<dyn Hittable>>
}

impl HittableList {
    pub fn new() -> Self {
        Self { objects: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.objects.clear();
    }

    pub fn add(&mut self, object: Arc<dyn Hittable>) {
        self.objects.push(object);
    }
}

impl Hittable for HittableList {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>> {
        let mut hit_anything: Option<HitRecord> = None;
        let mut closest_so_far = ray_t.max;

        // Iteramos sobre todos los objetos buscando la colisión más cercana a la cámara
        for object in &self.objects {
            let current_interval = Interval::new(ray_t.min, closest_so_far);
            if let Some(rec) = object.hit(ray, current_interval) {
                closest_so_far = rec.t;
                hit_anything = Some(rec);
            }
        }

        hit_anything
    }
}

