use crate::{materials::Lambertian, *};
use rt_core::{Point3, Ray, Vec3};

// Un material mock que implementa la interfaz para pruebas puntuales
#[derive(Debug)]
struct MockMaterial;

impl Material for MockMaterial {
    fn scatter(&self, _: &Ray, _: &HitRecord, _: &mut fastrand::Rng) -> Option<ScatterResult> { None }
}

#[test]
fn test_lambertian_attenuation_and_direction() {
    let albedo = Vec3::new(0.8, 0.3, 0.2);
    let mat = Lambertian::new(albedo);
    
    let ray_in = Ray::new(Point3::ZERO, Vec3::new(0.0, 0.0, -1.0));
    let mock_mat = MockMaterial;
    
    // Simulamos un impacto en el origen con una normal apuntando hacia +Y
    let rec = HitRecord {
        u: 0.0,
        v: 0.0,
        tangent: Vec3::ZERO,
        p: Point3::ZERO,
        normal: Vec3::Y,
        t: 1.0,
        front_face: true,
        material: &mock_mat,
    };

    let result = mat.scatter(&ray_in, &rec, &mut fastrand::Rng::new());

    assert!(result.is_some(), "Lambertian siempre debería dispersar el rayo");
    let ray_scattered = match result.unwrap() {
        ScatterResult::Diffuse { scattered } => scattered,
        other => panic!("Lambertian debe ser un rebote difuso, fue {:?}", other),
    };

    // 1. Validar conservación y fidelidad del color: con muestreo coseno,
    //    bsdf/pdf == albedo para la dirección muestreada
    let pdf = mat.scattering_pdf(&ray_in, &rec, ray_scattered.direction);
    let bsdf = mat.bsdf(&ray_in, &rec, ray_scattered.direction);
    assert!(pdf > 0.0, "La pdf de la dirección muestreada debe ser positiva");

    let attenuation = bsdf / pdf;
    assert!(
        (attenuation - albedo).abs().max_element() < 1e-5,
        "bsdf/pdf debe igualar el albedo. Esperado {:?}, real {:?}", albedo, attenuation
    );

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
