use rt_core::{Color, Ray, Vec3};

use crate::Material;
use crate::utils::*;

#[derive(Debug)]
pub struct Lambertian {
    pub albedo: Color,
}

impl Lambertian {
    pub fn new(albedo: Color) -> Self {
        Self{ albedo }
    }
}

impl Material for Lambertian {
    fn scatter(&self, _ray_in: &rt_core::Ray, rec: &crate::HitRecord) -> Option<(rt_core::Vec3, rt_core::Ray)> {
        let mut scatter_direction = rec.normal + random_unit_vector();

        if is_near_zero(&scatter_direction) {
            scatter_direction = rec.normal
        }

        let scattered_ray = Ray::new(rec.p, scatter_direction);

        Some((self.albedo, scattered_ray))
    }
}

#[derive(Debug)]
pub struct Metal {
    pub albedo: Color,
    pub fuzz: f32
}

impl Metal {
    pub fn new(albedo: Color, fuzz: f32) -> Self {
        Self{ albedo, fuzz }
    }
}

impl Material for Metal {
    fn scatter(&self, ray_in: &Ray, rec: &crate::HitRecord) -> Option<(Vec3, Ray)> {
        let reflected = reflect(ray_in.direction, rec.normal);

        let scatter_direction = reflected + self.fuzz * random_unit_vector();
        let scattered_ray = Ray::new(rec.p, scatter_direction);

        if scatter_direction.dot(rec.normal) > 0.0 {
            Some((self.albedo, scattered_ray))
        } else {
            None // Si el fuzz empujó el rayo hacia adentro de la geometría, la luz se absorbe
        }
    }
}

#[derive(Debug)]
pub struct Dielectric {
    pub refraction_index: f32, // Índice de refracción (ej: 1.5)
}

impl Dielectric {
    pub fn new(refraction_index: f32) -> Self {
        Self { refraction_index }
    }
}

impl Material for Dielectric {
    fn scatter(&self, ray_in: &Ray, rec: &crate::HitRecord) -> Option<(Vec3, Ray)> {
        let attenuation = Color::ONE;

        let ri = if rec.front_face {
            1.0/ &self.refraction_index
        } else {
            self.refraction_index
        };

        let cos_theta = (-ray_in.direction).dot(rec.normal).min(1.0);
        let sin_theta = (1.0- cos_theta * cos_theta).sqrt();

        let cannot_refract = ri * sin_theta > 1.0;
        
        let scatter_direction = if cannot_refract || reflectance(cos_theta, ri) > fastrand::f32() {
            // Caso A: Reflexión interna total o probabilidad alta según Schlick -> El rayo rebota como espejo
            reflect(ray_in.direction, rec.normal)
        } else {
            // Caso B: El rayo se refracta y atraviesa el material de forma física
            refract(ray_in.direction, rec.normal, ri)
        };

        Some((attenuation, Ray::new(rec.p, scatter_direction)))
    }
}
