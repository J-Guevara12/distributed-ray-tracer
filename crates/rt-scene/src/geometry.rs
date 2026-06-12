use std::sync::Arc;

use crate::*;

pub struct Sphere {
    pub center: Point3,
    pub radius: f32,
    pub material: Arc<dyn Material>,
}

pub struct Quad {
    pub material: Arc<dyn Material>,
    pub q: Point3,
    pub u: Vec3,
    pub v: Vec3,
    pub n: Vec3,
    pub d: f32,
}

impl Sphere {
    pub fn new(center: Point3, radius: f32, material: Arc<dyn Material>) -> Self {
        Self {
            center,
            radius,
            material,
        }
    }
}

impl Hittable for Sphere {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>> {
        let oc = self.center - ray.origin;
        let h = ray.direction.dot(oc);
        let c = oc.length_squared() - self.radius * self.radius;

        let discriminant = h * h - c;

        if discriminant < 0.0 {
            return None;
        }

        let sqrtd = discriminant.sqrt();

        let mut root = h - sqrtd;

        if !ray_t.surrounds(root) {
            root = h + sqrtd;
            if !ray_t.surrounds(root) {
                return None;
            }
        }

        let t = root;
        let p = ray.at(t);
        let outward_normal = (p - self.center) / self.radius;

        Some(HitRecord::new(
            ray,
            t,
            outward_normal,
            p,
            self.material.as_ref(),
        ))
    }
}

impl Quad {
    pub fn new(q: Point3, u: Vec3, v: Vec3, material: Arc<dyn Material>) -> Self {
        let n = u.cross(v).normalize();
        let d = n.dot(q);
        Self {
            q,
            u,
            v,
            n,
            d,
            material,
        }
    }

    pub fn is_interior(alpha: f32, betha: f32) -> bool {
        let unit_interval = Interval::new(0.0, 1.0);

        unit_interval.contains(alpha) && unit_interval.contains(betha)
    }
}

impl Hittable for Quad {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>> {
        let denom = self.n.dot(ray.direction);

        if denom.abs() < 1e-8 {
            return None;
        }

        let t = (self.d - self.n.dot(ray.origin)) / denom;

        if ray_t.contains(t) {
            return None;
        }

        let intersection = ray.at(t);
        let planar_vector = intersection - self.q;
        let alpha = self.n.dot(planar_vector.cross(self.u));
        let betha = self.n.dot(self.v.cross(planar_vector));

        if !Quad::is_interior(alpha, betha) {
            return None;
        }

        Some(HitRecord::new(
            ray,
            t,
            self.n,
            intersection,
            self.material.as_ref(),
        ))
    }
}
