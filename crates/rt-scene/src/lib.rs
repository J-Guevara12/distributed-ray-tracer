use rt_core::{Interval, Point3, Ray, Vec3, background::Background};
use std::sync::Arc;

pub use crate::aabb::Aabb;
pub use crate::materials::Material;

pub mod geometry;
pub mod hittable_list;
pub mod materials;
pub mod aabb;
pub mod bvh;
pub mod linear_scan;
pub mod primitive;
mod utils;

#[derive(Debug, Clone, Copy)]
pub struct HitRecord {
    pub p: Point3,
    pub normal: Vec3,
    pub t: f32,
    pub front_face: bool,
    /// Índice en el array de materiales de la escena. Un índice y no una
    /// referencia: elimina el lifetime del record y baja la primitiva de 48 a
    /// 32 bytes.
    pub material: u32,
}

impl HitRecord {
    pub fn new(
        ray: &Ray,
        t: f32,
        outward_normal: Vec3,
        p: Point3,
        material: u32,
    ) -> Self {
        // Determinar si el rayo viene de afuera o de adentro del objeto
        let front_face = ray.direction.dot(outward_normal) < 0.0;
        let normal = if front_face {
            outward_normal
        } else {
            -outward_normal
        };

        Self {
            p,
            normal,
            t,
            front_face,
            material,
        }
    }
}

/// Trabajo del recorrido, acumulado por hilo. Nunca atómicos: con 24 hilos un
/// `fetch_add` por nodo cuesta un orden de magnitud más que el recorrido mismo.
#[derive(Debug, Default, Clone, Copy)]
pub struct TraversalStats {
    /// Nodos cuyo AABB se testeó.
    pub node_visits: u64,
    /// Primitivas contra las que se hizo el test de intersección real.
    pub prim_tests: u64,
    /// De esos tests, los que devolvieron impacto. La diferencia importa para el
    /// roofline: un test que falla sale temprano y cuesta la mitad, así que sin
    /// este contador el conteo de FLOPs tendría que suponer la tasa de acierto.
    pub prim_hits: u64,
}

impl TraversalStats {
    pub fn merge(&mut self, other: &Self) {
        self.node_visits += other.node_visits;
        self.prim_tests += other.prim_tests;
        self.prim_hits += other.prim_hits;
    }
}

pub trait Hittable: Send + Sync {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord>;
    fn bounding_box(&self) -> Aabb;

    /// Igual que `hit`, contando el trabajo. Solo las estructuras de
    /// aceleración lo sobreescriben; para una primitiva no hay nada que contar.
    fn hit_counted(
        &self,
        ray: &Ray,
        ray_t: Interval,
        _stats: &mut TraversalStats,
    ) -> Option<HitRecord> {
        self.hit(ray, ray_t)
    }
}

/// Geometría más los materiales que sus primitivas indexan. El array vive
/// junto al mundo porque `HitRecord` solo guarda el índice.
pub struct Scene {
    pub world: Arc<dyn Hittable>,
    pub materials: Vec<Material>,
    pub background: Background,
}

#[cfg(test)]
mod tests;
