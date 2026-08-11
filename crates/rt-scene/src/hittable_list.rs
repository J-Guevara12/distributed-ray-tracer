use rt_core::dto::{MaterialDTO, ObjectDTO, ScenePayload};

use crate::{
    geometry::{PlanarShape, Sphere},
    materials::{Dielectric, DiffuseLight, Lambertian, Metal},
    *,
};

use crate::Hittable;
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub struct HittableList {
    pub objects: Vec<Arc<dyn Hittable>>,
    bbox: Aabb,
}

impl HittableList {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            bbox: Aabb::default()
        }
    }

    pub fn clear(&mut self) {
        self.objects.clear();
    }

    pub fn add(&mut self, object: Arc<dyn Hittable>) {
        self.bbox = Aabb::surrounding_box(self.bbox, object.bounding_box());
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
    fn bounding_box(&self) -> Aabb {
        self.bbox
    }
}

impl From<&ScenePayload> for HittableList {
    fn from(value: &ScenePayload) -> Self {
        let mut mundo = HittableList::new();
        let mut materials = HashMap::new();

        for (id, mat_dto) in &value.materials {
            let material: Arc<dyn Material> = match mat_dto {
                MaterialDTO::Lambertian { albedo } => Arc::new(Lambertian::new(*albedo)),
                MaterialDTO::Metal { albedo, fuzz } => Arc::new(Metal::new(*albedo, *fuzz)),
                MaterialDTO::Direlectric { refraction_index } => {
                    Arc::new(Dielectric::new(*refraction_index))
                }
                MaterialDTO::DiffuseLight { emit } => Arc::new(DiffuseLight::new(*emit)),
            };
            materials.insert(id.clone(), material);
        }
        for obj in &value.objects {
            match obj {
                ObjectDTO::Sphere {
                    center,
                    radius,
                    material,
                } => {
                    if let Some(mat) = materials.get(material) {
                        let sphere = Sphere::new(*center, *radius, Arc::clone(mat));

                        mundo.add(Arc::new(sphere));
                    } else {
                        eprintln!(
                            "Warning: El material '{}' no fue encontrado para la esfera.",
                            material
                        );
                    }
                }
                ObjectDTO::Quad { q, u, v, material } => {
                    if let Some(mat) = materials.get(material) {
                        let quad = PlanarShape::new(
                            *q,
                            *u,
                            *v,
                            geometry::PlanarType::Quad,
                            Arc::clone(mat),
                        );

                        mundo.add(Arc::new(quad));
                    }
                }
                ObjectDTO::Triangle { q, u, v, material } => {
                    if let Some(mat) = materials.get(material) {
                        let triangle = PlanarShape::new(
                            *q,
                            *u,
                            *v,
                            geometry::PlanarType::Triangle,
                            Arc::clone(mat),
                        );

                        mundo.add(Arc::new(triangle));
                    }
                }
                ObjectDTO::Elipse { q, u, v, material } => {
                    if let Some(mat) = materials.get(material) {
                        let elipse = PlanarShape::new(
                            *q,
                            *u,
                            *v,
                            geometry::PlanarType::Elipse,
                            Arc::clone(mat),
                        );

                        mundo.add(Arc::new(elipse));
                    }
                }
            }
        }

        mundo
    }
}
