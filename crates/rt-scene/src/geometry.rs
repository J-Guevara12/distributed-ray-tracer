

use crate::*;

pub struct Sphere {
    pub center: Point3,
    pub radius: f32,
    pub material: u32,
}

pub enum PlanarType {
    Quad,
    Triangle,
    Elipse,
}

pub struct PlanarShape {
    pub q: Point3,
    pub u: Vec3,
    pub v: Vec3,
    pub n: Vec3,
    pub w: Vec3,
    pub d: f32,
    pub primitive_type: PlanarType,
    pub material: u32,
    pub bbox: Aabb
}

impl Sphere {
    pub fn new(center: Point3, radius: f32, material: u32) -> Self {
        Self {
            center,
            radius,
            material,
        }
    }
}

impl Hittable for Sphere {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord> {
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
            self.material,
        ))
    }
    fn bounding_box(&self) -> Aabb {
        let min = self.center - self.radius;
        let max = self.center + self.radius;

        Aabb { min, max }
    }
}

impl PlanarShape {
    pub fn new(
        q: Point3,
        u: Vec3,
        v: Vec3,
        primitive_type: PlanarType,
        material: u32,
    ) -> Self {
        let w = u.cross(v);
        let n = w.normalize();
        let d = n.dot(q);
        let w = w / (w.dot(w));

        let raw_bbox = match primitive_type {
            PlanarType::Quad => {
                let box_a = Aabb::from_points(q, q+u+v);
                let box_b = Aabb::from_points(q + u, q + v);
                Aabb::surrounding_box(box_a, box_b)
            },
            PlanarType::Triangle => {
                let box_a = Aabb::from_points(q, q+u+v);
                let box_b = Aabb::from_points(q + u, q + v);
                Aabb::surrounding_box(box_a, box_b)
            }
            PlanarType::Elipse => {
                let box_a = Aabb::from_points(q - u - v, q + u + v);
                let box_b = Aabb::from_points(q - u + v, q + u - v);
                Aabb::surrounding_box(box_a, box_b)
            }
        };

        let bbox = raw_bbox.pad_delta();

        Self {
            q,
            u,
            v,
            n,
            w,
            d,
            primitive_type,
            material,
            bbox
        }
    }

    pub fn is_interior(&self, alpha: f32, betha: f32) -> bool {
        let unit_interval = Interval::new(0.0, 1.0);

        match self.primitive_type {
            PlanarType::Quad => unit_interval.contains(alpha) && unit_interval.contains(betha),
            PlanarType::Triangle => {
                alpha > 0.0 && betha > 0.0 && unit_interval.contains(alpha + betha)
            }
            PlanarType::Elipse => unit_interval.contains(alpha * alpha + betha * betha),
        }
    }
}

impl Hittable for PlanarShape {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord> {
        let denom = self.n.dot(ray.direction);

        if denom.abs() < 1e-8 {
            return None;
        }

        let t = (self.d - self.n.dot(ray.origin)) / denom;

        if !ray_t.contains(t) {
            return None;
        }

        let intersection = ray.at(t);
        let planar_vector = intersection - self.q;
        let alpha = self.w.dot(planar_vector.cross(self.v));
        let betha = self.w.dot(self.u.cross(planar_vector));

        if !self.is_interior(alpha, betha) {
            return None;
        }

        Some(HitRecord::new(
            ray,
            t,
            self.n,
            intersection,
            self.material,
        ))
    }

    fn bounding_box(&self) -> Aabb {
        self.bbox
    }
}
