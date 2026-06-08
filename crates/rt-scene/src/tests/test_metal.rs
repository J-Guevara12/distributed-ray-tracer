use crate::{HitRecord, Material, materials::Metal};
use glam::Vec3A;
use rt_core::{Point3, Ray};

#[derive(Debug)]
struct MockMaterial;
impl Material for MockMaterial { fn scatter(&self, _: &Ray, _: &HitRecord) -> Option<(Vec3A, Ray)> { None } }

#[test]
fn test_perfect_metal_reflection_at_45_degrees() {
    let albedo = Vec3A::ONE; // Metal blanco perfecto
    let espejo_perfecto = Metal::new(albedo, 0.0); // Fuzz = 0.0
    
    // Rayo entrante a 45° cayendo hacia el origen
    let ray_in = Ray::new(Point3::new(-1.0, 1.0, 0.0), Vec3A::new(1.0, -1.0, 0.0));
    let mock_mat = MockMaterial;
    let rec = HitRecord {
        p: Point3::ZERO,
        normal: Vec3A::Y, // Superficie horizontal mirando hacia arriba
        t: 1.0,
        front_face: true,
        material: &mock_mat,
    };

    let (_, ray_scattered) = espejo_perfecto.scatter(&ray_in, &rec).unwrap();

    // Esperado: Debe salir rebotado perfectamente a 45° hacia arriba
    let direccion_esperada = Vec3A::new(1.0, 1.0, 0.0).normalize();
    let direccion_real = ray_scattered.direction.normalize();

    assert!(
        (direccion_real.x - direccion_esperada.x).abs() < 1e-5,
        "Ángulo de reflexión incorrecto en X. Esperado: {}, Real: {}", direccion_esperada.x, direccion_real.x
    );
    assert!(
        (direccion_real.y - direccion_esperada.y).abs() < 1e-5,
        "Ángulo de reflexión incorrecto en Y. Esperado: {}, Real: {}", direccion_esperada.y, direccion_real.y
    );
}

#[test]
fn test_metal_fuzzy_absorption_edge_case() {
    let albedo = Vec3A::ONE;
    // Un metal extremadamente rugoso (fuzz = 1.0)
    let metal_rugoso = Metal::new(albedo, 1.0);
    
    // Un rayo que golpea de forma muy rasante (casi horizontal)
    let ray_in = Ray::new(Point3::new(-1.0, 0.01, 0.0), Vec3A::new(1.0, -0.01, 0.0));
    let mock_mat = MockMaterial;
    let rec = HitRecord {
        p: Point3::ZERO,
        normal: Vec3A::Y,
        t: 1.0,
        front_face: true,
        material: &mock_mat,
    };

    // Debido a la alta rugosidad, probabilísticamente muchas muestras intentarán rebotar
    // por debajo de la normal (Y < 0). Hacemos un bucle para cazar ese caso.
    let mut absorbido = false;
    for _ in 0..100 {
        if metal_rugoso.scatter(&ray_in, &rec).is_none() {
            absorbido = true;
            break;
        }
    }

    assert!(
        absorbido, 
        "Un metal con fuzz = 1.0 en un impacto rasante debió haber absorbido el rayo al menos una vez"
    );
}
