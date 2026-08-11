use rt_core::{Interval, Point3, Ray};

#[derive(Clone, Copy, Default)]
pub struct Aabb {
    pub x: Interval,
    pub y: Interval,
    pub z: Interval,
}

impl Aabb {
    pub fn hit(&self, ray: Ray, mut ray_t: Interval) -> bool {
        for axis in 0..3 {
            let ax_interval = match axis {
                0 => self.x,
                1 => self.y,
                _ => self.z,
            };

            let inv_d = 1.0/ray.direction[axis];
            let origin = ray.origin[axis];

            let mut t0 = (ax_interval.min - origin) * inv_d;
            let mut t1 = (ax_interval.max - origin) * inv_d;

            if inv_d < 0.0 {
                std::mem::swap(&mut t0, &mut t1);
            }

            if t0 > ray_t.min { ray_t.min = t0; }
            if t1 < ray_t.max { ray_t.max = t1; }

            if ray_t.max <= ray_t.min {
                return false
            }
        }

        true
    }

    pub fn surrounding_box(left: Aabb, right: Aabb) -> Self {
        let x = Interval::new(left.x.min.min(right.x.min), left.x.max.max(right.x.max));
        let y = Interval::new(left.y.min.min(right.y.min), left.y.max.max(right.y.max));
        let z = Interval::new(left.z.min.min(right.z.min), left.z.max.max(right.z.max));

        Self { x, y, z }
    
    }

    pub fn pad_delta(&self) -> Self {
        let delta = 0.0001; // Pequeño margen para dar volumen al AABB
        
        let new_x = if self.x.size() < delta { self.x.expand(delta) } else { self.x };
        let new_y = if self.y.size() < delta { self.y.expand(delta) } else { self.y };
        let new_z = if self.z.size() < delta { self.z.expand(delta) } else { self.z };

        Self { x: new_x, y: new_y, z: new_z }
    }

    pub fn from_points(a: Point3, b: Point3) -> Self {
        Self {
            x: Interval::new(a.x.min(b.x), a.x.max(b.x)),
            y: Interval::new(a.y.min(b.y), a.y.max(b.y)),
            z: Interval::new(a.z.min(b.z), a.z.max(b.z)),
        }
    }
}
