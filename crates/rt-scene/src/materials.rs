use fastrand::Rng;
use rt_core::{Color, Point3, Ray};

use crate::HitRecord;
use crate::utils::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    Lambertian { albedo: Color },
    Metal { albedo: Color, fuzz: f32 },
    Dielectric { refraction_index: f32 },
    DiffuseLight { emit: Color },
}

impl Material {
    pub fn scatter(&self, ray_in: &Ray, rec: &HitRecord, rng: &mut Rng) -> Option<(Color, Ray)> {
        match *self {
            Material::Lambertian { albedo } => {
                let mut scatter_direction = rec.normal + random_unit_vector(rng);

                if is_near_zero(&scatter_direction) {
                    scatter_direction = rec.normal
                }

                Some((albedo, Ray::new(rec.p, scatter_direction)))
            }

            Material::Metal { albedo, fuzz } => {
                let reflected = reflect(ray_in.direction, rec.normal);
                let scatter_direction = reflected + fuzz * random_unit_vector(rng);

                if scatter_direction.dot(rec.normal) > 0.0 {
                    Some((albedo, Ray::new(rec.p, scatter_direction)))
                } else {
                    // El fuzz empujó el rayo hacia adentro de la geometría: se absorbe
                    None
                }
            }

            Material::Dielectric { refraction_index } => {
                let ri = if rec.front_face {
                    1.0 / refraction_index
                } else {
                    refraction_index
                };

                let cos_theta = (-ray_in.direction).dot(rec.normal).min(1.0);
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

                let cannot_refract = ri * sin_theta > 1.0;

                let scatter_direction = if cannot_refract || reflectance(cos_theta, ri) > rng.f32() {
                    reflect(ray_in.direction, rec.normal)
                } else {
                    refract(ray_in.direction, rec.normal, ri)
                };

                Some((Color::ONE, Ray::new(rec.p, scatter_direction)))
            }

            Material::DiffuseLight { .. } => None,
        }
    }

    pub fn emitted(&self, _u: f32, _v: f32, _p: Point3) -> Color {
        match *self {
            Material::DiffuseLight { emit } => emit,
            _ => Color::ZERO,
        }
    }
}
