use rt_core::{Color, Ray, Vec3};

use crate::Material;

pub fn random_unit_vector() -> Vec3 {
    loop {
        let p = Vec3::new(
            fastrand::f32() * 2.0 - 1.0,
            fastrand::f32() * 2.0 - 1.0,
            fastrand::f32() * 2.0 - 1.0,
        );
        
        let len_sq = p.length_squared();
        // Validamos el edge case de que no sea cero y caiga dentro de la esfera
        if len_sq > 1e-8 && len_sq <= 1.0 {
            return p / len_sq.sqrt();
        }
    }
}
fn is_near_zero(v: &Color) -> bool {
    let s = 1e-8;
    v.x.abs() < s && v.y.abs() < s && v.z.abs() < s
}

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
