use fastrand::Rng;
use rt_core::{Vec3, Color};

pub fn random_unit_vector(rng: &mut Rng) -> Vec3 {
    loop {
        let p = Vec3::new(
            rng.f32() * 2.0 - 1.0,
            rng.f32() * 2.0 - 1.0,
            rng.f32() * 2.0 - 1.0,
        );
        
        let len_sq = p.length_squared();
        // Validamos el edge case de que no sea cero y caiga dentro de la esfera
        if len_sq > 1e-8 && len_sq <= 1.0 {
            return p / len_sq.sqrt();
        }
    }
}
pub fn is_near_zero(v: &Color) -> bool {
    let s = 1e-8;
    v.x.abs() < s && v.y.abs() < s && v.z.abs() < s
}

pub fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    v - 2.0 * v.dot(n) * n
}

pub fn refract(uv: Vec3, n: Vec3, etai_over_etat: f32) -> Vec3 {
    let cos_theta = (-uv).dot(n).min(1.0);
    let r_out_perp = etai_over_etat * (uv + cos_theta * n);
    let r_out_parallel = -(1.0 - r_out_perp.length_squared()).abs().sqrt() * n;
    r_out_perp + r_out_parallel
}

// Aproximación de Schlick para la reflectancia variable según el ángulo de visión
pub fn reflectance(cosine: f32, refraction_index: f32) -> f32 {
    let mut r0 = (1.0 - refraction_index) / (1.0 + refraction_index);
    r0 = r0 * r0;
    r0 + (1.0 - r0) * (1.0 - cosine).powi(5)
}

