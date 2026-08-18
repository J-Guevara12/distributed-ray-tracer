use std::sync::Arc;

use rt_core::{Interval, Vec3};

use crate::{Hittable, aabb::Aabb};

pub struct BvhNode {
    left: Arc<dyn Hittable>,
    right: Arc<dyn Hittable>,
    bbox: Aabb
}

fn longest_axis(objects: &Vec<Arc<dyn Hittable>>) -> u32 {
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

impl BvhNode {
    pub fn new(mut objects: Vec<Arc<dyn Hittable>>) -> Self {
        let axis = longest_axis(&objects);

        let comparator = |a: &Arc<dyn Hittable>, b: &Arc<dyn Hittable>| {
            let box_a = a.bounding_box();
            let box_b = b.bounding_box();

            let (min_a, min_b) = match axis {
                0 => (box_a.x.min, box_b.x.min),
                1 => (box_a.y.min, box_b.y.min),
                _ => (box_a.z.min, box_b.z.min)
            };

            min_a.partial_cmp(&min_b).unwrap_or(std::cmp::Ordering::Equal)
        };

        let object_span = objects.len();
        let (left, right): (Arc<dyn Hittable>, Arc<dyn Hittable>) = match object_span {
            0 => panic!("No se puede construir un BvhNode con 0 objetos"),
            1 => {
                (Arc::clone(&objects[0]), Arc::clone(&objects[0]))
            }
            2 => {
                objects.sort_by(comparator);
                (Arc::clone(&objects[0]), Arc::clone(&objects[1]))
            }
            _ => {
                objects.sort_by(comparator);
                let mid = object_span/2;

                let right_objects = objects.split_off(mid);
                let left_objects = objects;

                (
                    Arc::new(BvhNode::new(left_objects)),
                    Arc::new(BvhNode::new(right_objects))
                )
            }
        };
        let bbox = Aabb::surrounding_box(left.bounding_box(), right.bounding_box());

        Self { left, right, bbox }
    }
}

impl Hittable for BvhNode {
    fn hit(&self, ray: &rt_core::Ray, ray_t: rt_core::Interval) -> Option<crate::HitRecord<'_>> {
        if !(self.bbox.hit(*ray, ray_t)){
            return None
        }

        let hit_left = self.left.hit(ray, ray_t);
        let mut current_max = ray_t.max;

        if let Some(ref rec) = hit_left {
            current_max = rec.t
        }

        let hit_right = self.right.hit(ray, Interval::new(ray_t.min, current_max));
        if hit_right.is_some() {
            hit_right
        } else {
            hit_left
        }
    }

    fn bounding_box(&self) -> Aabb {
        self.bbox
    }
}
