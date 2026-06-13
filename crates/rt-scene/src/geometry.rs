use std::sync::Arc;

use rt_core::dto::{MaterialDTO, ObjectDTO, ScenePayload};
use std::collections::HashMap;

use crate::materials::{Dielectric, Lambertian, Metal};
use crate::*;

#[derive(Clone)]
pub struct Sphere {
    pub center: Point3,
    pub radius: f32,
    pub material: Arc<dyn Material>,
    /// Desplazamiento del centro durante el intervalo [0,1] del obturador (motion blur).
    pub velocity: Vec3,
}

#[derive(Clone)]
pub struct Quad {
    pub material: Arc<dyn Material>,
    pub q: Point3,
    pub u: Vec3,
    pub v: Vec3,
    pub n: Vec3,
    pub w: Vec3,
    pub d: f32,
}

impl Sphere {
    pub fn new(center: Point3, radius: f32, material: Arc<dyn Material>) -> Self {
        Self {
            center,
            radius,
            material,
            velocity: Vec3::ZERO,
        }
    }

    /// Esfera en movimiento: el centro se desplaza `velocity` durante el obturador.
    pub fn moving(center: Point3, velocity: Vec3, radius: f32, material: Arc<dyn Material>) -> Self {
        Self {
            center,
            radius,
            material,
            velocity,
        }
    }

    #[inline]
    fn center_at(&self, time: f32) -> Point3 {
        self.center + self.velocity * time
    }

    /// UV esférico estándar (equirectangular) a partir de la normal unitaria,
    /// más la tangente (dirección de u creciente, rotación sobre Y).
    fn uv_and_tangent(outward_normal: Vec3) -> (f32, f32, Vec3) {
        use std::f32::consts::PI;

        let theta = (-outward_normal.y).clamp(-1.0, 1.0).acos();
        let phi = (-outward_normal.z).atan2(outward_normal.x) + PI;

        let u = phi / (2.0 * PI);
        let v = theta / PI;

        // d(p)/du apunta en la dirección de phi creciente; degenera en los polos
        let mut tangent = Vec3::new(-outward_normal.z, 0.0, outward_normal.x);
        if tangent.length_squared() < 1e-12 {
            tangent = Vec3::X;
        } else {
            tangent = tangent.normalize();
        }

        (u, v, tangent)
    }
}

impl Hittable for Sphere {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>> {
        let center = self.center_at(ray.time);
        let oc = center - ray.origin;
        // Cuadrática general: válida también para direcciones no unitarias
        // (los rayos transformados por una instancia pueden venir escalados).
        let a = ray.direction.length_squared();
        let h = ray.direction.dot(oc);
        let c = oc.length_squared() - self.radius * self.radius;

        let discriminant = h * h - a * c;

        if discriminant < 0.0 {
            return None;
        }

        let sqrtd = discriminant.sqrt();

        let mut root = (h - sqrtd) / a;

        if !ray_t.surrounds(root) {
            root = (h + sqrtd) / a;
            if !ray_t.surrounds(root) {
                return None;
            }
        }

        let t = root;
        let p = ray.at(t);
        let outward_normal = (p - center) / self.radius;
        let (u, v, tangent) = Self::uv_and_tangent(outward_normal);

        Some(HitRecord::with_uv(
            ray,
            t,
            outward_normal,
            p,
            self.material.as_ref(),
            u,
            v,
            tangent,
        ))
    }

    fn bounding_box(&self) -> Aabb {
        // radius.abs() para soportar esferas de radio negativo (vidrio hueco)
        let radius_vector = Vec3::splat(self.radius.abs());
        let box_start = Aabb::new(self.center - radius_vector, self.center + radius_vector);

        if self.velocity.length_squared() == 0.0 {
            return box_start;
        }

        // Caja que cubre la posición de la esfera durante todo el obturador
        let end = self.center + self.velocity;
        let box_end = Aabb::new(end - radius_vector, end + radius_vector);
        box_start.union(box_end)
    }
}

impl Quad {
    pub fn new(q: Point3, u: Vec3, v: Vec3, material: Arc<dyn Material>) -> Self {
        let w = u.cross(v);
        let n = w.normalize();
        let d = n.dot(q);
        let w = w / (w.dot(w));
        Self {
            q,
            u,
            v,
            n,
            w,
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

        if !ray_t.contains(t) {
            return None;
        }

        let intersection = ray.at(t);
        let planar_vector = intersection - self.q;
        let alpha = self.w.dot(planar_vector.cross(self.v));
        let betha = self.w.dot(self.u.cross(planar_vector));

        if !Quad::is_interior(alpha, betha) {
            return None;
        }

        Some(HitRecord::with_uv(
            ray,
            t,
            self.n,
            intersection,
            self.material.as_ref(),
            alpha,
            betha,
            self.u.normalize(),
        ))
    }

    fn bounding_box(&self) -> Aabb {
        Aabb::from_points(&[self.q, self.q + self.u, self.q + self.v, self.q + self.u + self.v])
            .pad(1e-4)
    }
}

/// Geometría concreta almacenada por valor: permite guardar los objetos en
/// arreglos contiguos (cache-friendly) y despachar estáticamente, sin el
/// puntero + vtable de `Arc<dyn Hittable>`.
#[derive(Clone)]
pub enum Primitive {
    Sphere(Sphere),
    Quad(Quad),
    Triangle(crate::mesh::Triangle),
    /// Malla con su propio BVH interno (compartible entre instancias).
    Mesh(Arc<crate::mesh::Mesh>),
}

impl Hittable for Primitive {
    #[inline]
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>> {
        match self {
            Primitive::Sphere(sphere) => sphere.hit(ray, ray_t),
            Primitive::Quad(quad) => quad.hit(ray, ray_t),
            Primitive::Triangle(triangle) => triangle.hit(ray, ray_t),
            Primitive::Mesh(mesh) => mesh.hit(ray, ray_t),
        }
    }

    fn bounding_box(&self) -> Aabb {
        match self {
            Primitive::Sphere(sphere) => sphere.bounding_box(),
            Primitive::Quad(quad) => quad.bounding_box(),
            Primitive::Triangle(triangle) => triangle.bounding_box(),
            Primitive::Mesh(mesh) => mesh.bounding_box(),
        }
    }
}

/// Construye la lista plana de primitivas de una escena, resolviendo los
/// materiales por id.
pub fn primitives_from_scene(payload: &ScenePayload) -> Vec<Primitive> {
    let mut materials: HashMap<String, Arc<dyn Material>> = HashMap::new();

    for (id, mat_dto) in &payload.materials {
        let material: Arc<dyn Material> = match mat_dto {
            MaterialDTO::Lambertian { albedo } => Arc::new(Lambertian::new(*albedo)),
            MaterialDTO::Metal { albedo, fuzz } => Arc::new(Metal::new(*albedo, *fuzz)),
            MaterialDTO::Direlectric { refraction_index } => {
                Arc::new(Dielectric::new(*refraction_index))
            }
        };
        materials.insert(id.clone(), material);
    }

    let mut primitives = Vec::with_capacity(payload.objects.len());

    for obj in &payload.objects {
        match obj {
            ObjectDTO::Sphere {
                center,
                radius,
                material,
            } => {
                if let Some(mat) = materials.get(material) {
                    primitives.push(Primitive::Sphere(Sphere::new(*center, *radius, Arc::clone(mat))));
                } else {
                    eprintln!(
                        "Warning: El material '{}' no fue encontrado para la esfera.",
                        material
                    );
                }
            }
            ObjectDTO::Quad { q, u, v, material } => {
                if let Some(mat) = materials.get(material) {
                    primitives.push(Primitive::Quad(Quad::new(*q, *u, *v, Arc::clone(mat))));
                } else {
                    eprintln!(
                        "Warning: El material '{}' no fue encontrado para el quad.",
                        material
                    );
                }
            }
        }
    }

    primitives
}
