use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{Point3, Vec3};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenePayload {
    pub materials: HashMap<String, MaterialDTO>,
    pub objects: Vec<ObjectDTO>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MaterialDTO {
    #[serde(rename = "lambertian")]
    Lambertian { albedo: Vec3 },
    #[serde(rename = "metal")]
    Metal { albedo: Vec3, fuzz: f32 },
    #[serde(rename = "dielectric")]
    Direlectric { refraction_index: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ObjectDTO {
    #[serde(rename = "sphere")]
    Sphere {
        center: Point3,
        radius: f32,
        material: String,
    },
}

impl ScenePayload {
    pub fn new(materials: HashMap<String, MaterialDTO>, objects: Vec<ObjectDTO>) -> Self {
        Self { materials, objects }
    }
}

impl Default for ScenePayload {
    fn default() -> Self {
        let material_ground = MaterialDTO::Lambertian {
            albedo: Vec3::new(0.0, 0.9, 0.2),
        };
        let material_front = MaterialDTO::Lambertian {
            albedo: Vec3::new(1.0, 0.0, 0.2),
        };
        let material_left = MaterialDTO::Metal {
            albedo: Vec3::new(1.0, 1.0, 1.0),
            fuzz: 0.0,
        };
        let material_right = MaterialDTO::Metal {
            albedo: Vec3::new(0.0, 0.5, 0.9),
            fuzz: 0.4,
        };
        let material_up_out = MaterialDTO::Direlectric {
            refraction_index: 1.5,
        };
        let material_up_in = MaterialDTO::Direlectric {
            refraction_index: 1.0 / 1.5,
        };

        let mut materials = HashMap::new();

        materials.insert("ground".to_string(), material_ground);
        materials.insert("front".to_string(), material_front);
        materials.insert("left".to_string(), material_left);
        materials.insert("right".to_string(), material_right);
        materials.insert("up_out".to_string(), material_up_out);
        materials.insert("up_in".to_string(), material_up_in);

        let objects = vec![
            ObjectDTO::Sphere {
                center: Point3::new(0.0, -100.5, -1.0),
                radius: 100.0,
                material: "ground".to_string(),
            },
            ObjectDTO::Sphere {
                center: Point3::new(0.0, 0.0, -1.0),
                radius: 0.5,
                material: "front".to_string(),
            },
            ObjectDTO::Sphere {
                center: Point3::new(-1.3, 0.0, -1.2),
                radius: 0.5,
                material: "left".to_string(),
            },
            ObjectDTO::Sphere {
                center: Point3::new(1.3, 0.0, -1.2),
                radius: 0.5,
                material: "right".to_string(),
            },
            ObjectDTO::Sphere {
                center: Point3::new(0.0, 0.6, -1.0),
                radius: 0.5,
                material: "up_out".to_string(),
            },
            ObjectDTO::Sphere {
                center: Point3::new(0.0, 0.6, -1.0),
                radius: 0.2,
                material: "up_in".to_string(),
            },
        ];
        Self::new(materials, objects)
    }
}
