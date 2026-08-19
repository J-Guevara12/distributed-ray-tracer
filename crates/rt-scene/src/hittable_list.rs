use rt_core::dto::{MaterialDTO, ObjectDTO, ScenePayload};

use crate::{
    primitive::Primitive,
    geometry::{PlanarShape, Sphere},
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
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord> {
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

/// Geometría más el array de materiales que sus primitivas indexan.
pub struct SceneData {
    pub objects: Vec<Primitive>,
    pub materials: Vec<Material>,
}

impl From<&ScenePayload> for SceneData {
    fn from(value: &ScenePayload) -> Self {
        let mut mundo: Vec<Primitive> = Vec::with_capacity(value.objects.len());
        let mut palette = Vec::with_capacity(value.materials.len());
        let mut materials = HashMap::new();

        for (id, mat_dto) in &value.materials {
            let material = match mat_dto {
                MaterialDTO::Lambertian { albedo } => Material::Lambertian { albedo: *albedo },
                MaterialDTO::Metal { albedo, fuzz } => Material::Metal {
                    albedo: *albedo,
                    fuzz: *fuzz,
                },
                MaterialDTO::Direlectric { refraction_index } => Material::Dielectric {
                    refraction_index: *refraction_index,
                },
                MaterialDTO::DiffuseLight { emit } => Material::DiffuseLight { emit: *emit },
            };
            palette.push(material);
            materials.insert(id.clone(), (palette.len() - 1) as u32);
        }
        for obj in &value.objects {
            match obj {
                ObjectDTO::Sphere {
                    center,
                    radius,
                    material,
                } => {
                    if let Some(mat) = materials.get(material) {
                        let sphere = Sphere::new(*center, *radius, *mat);

                        mundo.push(sphere.into());
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
                            *mat,
                        );

                        mundo.push(quad.into());
                    }
                }
                ObjectDTO::Triangle { q, u, v, material } => {
                    if let Some(mat) = materials.get(material) {
                        let triangle = PlanarShape::new(
                            *q,
                            *u,
                            *v,
                            geometry::PlanarType::Triangle,
                            *mat,
                        );

                        mundo.push(triangle.into());
                    }
                }
                ObjectDTO::Elipse { q, u, v, material } => {
                    if let Some(mat) = materials.get(material) {
                        let elipse = PlanarShape::new(
                            *q,
                            *u,
                            *v,
                            geometry::PlanarType::Elipse,
                            *mat,
                        );

                        mundo.push(elipse.into());
                    }
                }
            }
        }

        Self { objects: mundo, materials: palette }
    }
}
