use rt_core::{Interval, Point3, Ray, Vec3};

#[derive(Clone, Copy, Default)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn hit(&self, ray: &Ray, ray_t: Interval) -> bool {
        let t0 = (self.min - ray.origin) * ray.inv_dir;
        let t1 = (self.max - ray.origin) * ray.inv_dir;

        let t_near = t0.min(t1);                          // el swap, sin rama
        let t_far  = t0.max(t1);

        let t_enter = t_near.max_element().max(ray_t.min);
        let t_exit  = t_far.min_element().min(ray_t.max);

        t_enter <= t_exit
    }

    pub fn surrounding_box(left: Aabb, right: Aabb) -> Self {
        let min = left.min.min(right.min);
        let max = left.max.max(right.max);

        Self { min, max }
    
    }

    pub fn pad_delta(&self) -> Self {
        let delta = Vec3::splat(0.0001);
        let degenerate = (self.max - self.min).cmplt(delta);
        let pad = Vec3::select(degenerate, delta, Vec3::ZERO);

        Self { min: self.min - pad, max: self.max + pad }
    }

    pub fn from_points(a: Point3, b: Point3) -> Self {
        Self {
            min: a.min(b),
            max: a.max(b),
        }
    }
}
