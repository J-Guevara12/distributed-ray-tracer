use crate::*;
use rt_core::sampler::IndependentSampler;
use rt_core::{Point3, Ray, Vec3};

// Un material mock que implementa la interfaz para pruebas puntuales


#[test]
fn test_lambertian_attenuation_and_direction() {
    let albedo = Vec3::new(0.8, 0.3, 0.2);
    let mat = Material::Lambertian { albedo: albedo };
    
    let ray_in = Ray::new(Point3::ZERO, Vec3::new(0.0, 0.0, -1.0));
    
    // Simulamos un impacto en el origen con una normal apuntando hacia +Y
    let rec = HitRecord {
        p: Point3::ZERO,
        normal: Vec3::Y,
        t: 1.0,
        front_face: true,
        material: 0,
    };

    let result = mat.scatter(&ray_in, &rec, &mut IndependentSampler::with_seed(0));
    
    assert!(result.is_some(), "Lambertian siempre debería dispersar el rayo");
    let (attenuation, ray_scattered) = result.unwrap();

    // 1. Validar conservación y fidelidad del color
    assert_eq!(attenuation, albedo, "La atenuación debe ser igual al albedo");

    // 2. Validar origen del rayo secundario
    assert_eq!(ray_scattered.origin, rec.p, "El rayo dispersado debe nacer en el punto de impacto");

    // 3. Edge Case: El rayo dispersado debe salir hacia el hemisferio exterior
    let dot_product = ray_scattered.direction.dot(rec.normal);
    assert!(
        dot_product > 0.0, 
        "El rayo difuso se dispersó hacia adentro de la superficie (dot = {})", dot_product
    );
    
    // 4. Asegurar que la dirección no sea un vector roto (NaN o infinito)
    assert!(ray_scattered.direction.is_finite(), "La dirección del rayo no es finita");
}
