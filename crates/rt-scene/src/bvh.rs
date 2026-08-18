use std::sync::Arc;

use rt_core::{Interval, Vec3};

use crate::{Hittable, aabb::Aabb};

pub struct BvhNode {
    left: Arc<dyn Hittable>,
    right: Arc<dyn Hittable>,
    bbox: Aabb,
    axis: usize,
}

fn longest_axis(objects: &Vec<Arc<dyn Hittable>>) -> usize {
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);

    for object in objects {
        let aabb = object.bounding_box();

        let c = Vec3::new(
            0.5 * (aabb.x.min + aabb.x.max),
            0.5 * (aabb.y.min + aabb.y.max),
            0.5 * (aabb.z.min + aabb.z.max)
        );

        low = low.min(c);
        high = high.max(c);
    }
    let extent = high - low;
    

    if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    }
}

// Ordena por el mínimo de la caja, no por el centroide. Sobre un eje que casi
// no separa objetos, el centroide produce un orden espacialmente arbitrario
// (en B2, ordenar por centroide en Y equivale a ordenar por radio, y cuesta un
// 33%). El mínimo empata en ese caso y el sort estable conserva el orden de
// entrada, que suele venir agrupado espacialmente.
fn sort_key(object: &Arc<dyn Hittable>, axis: usize) -> f32 {
    let b = object.bounding_box();
    match axis {
        0 => b.x.min,
        1 => b.y.min,
        _ => b.z.min,
    }
}

impl BvhNode {
    pub fn build(mut objects: Vec<Arc<dyn Hittable>>) -> Arc<dyn Hittable> {
        match objects.len() {
            0 => panic!("No se puede construir un BvhNode con 0 objetos"),
            1 => objects.pop().unwrap(),
            _ => {
                let axis = longest_axis(&objects);
                objects.sort_by(|a, b| sort_key(a, axis).total_cmp(&sort_key(b, axis)));

                let right_objects = objects.split_off(objects.len()/2);

                let left = Self::build(objects);
                let right = Self::build(right_objects);

                let bbox = Aabb::surrounding_box(left.bounding_box(), right.bounding_box());

                Arc::new(Self { left, right, bbox, axis })
            }
        }

    }
}

impl Hittable for BvhNode {
    fn hit(&self, ray: &rt_core::Ray, ray_t: rt_core::Interval) -> Option<crate::HitRecord<'_>> {
        if !(self.bbox.hit(*ray, ray_t)){
            return None
        }

        let (near, far) = match ray.direction[self.axis] < 0.0 {
            true => (&self.right, &self.left),
            false => (&self.left, &self.right),
        };

        let hit_near = near.hit(ray, ray_t);
        let max = hit_near.as_ref().map_or(ray_t.max, |rec| rec.t);

        let hit_far = far.hit(ray, Interval::new(ray_t.min, max));

        hit_far.or(hit_near)
    }

    fn bounding_box(&self) -> Aabb {
        self.bbox
    }
}
