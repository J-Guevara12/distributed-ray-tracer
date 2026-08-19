use rt_core::{Color, Point3, Ray, Vec3};

use crate::{HitRecord, Material};

// Función auxiliar para generar un HitRecord dummy necesario para la firma de scatter
fn setup_dummy_hit_record() -> HitRecord {
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));

    HitRecord::new(
        &ray,
        1.0,
        Vec3::new(0.0, 0.0, 1.0),    // Normal
        Point3::new(0.0, 0.0, -1.0), // Punto de intersección
        0,
    )
}

#[test]
fn test_scatter_returns_none() {
    // Un material emisivo puro no debe dispersar la luz, debe absorber el rayo
    let light_color = Color::new(5.0, 5.0, 5.0);
    let light_material = Material::DiffuseLight { emit: light_color };

    let ray_in = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));
    let rec = setup_dummy_hit_record();

    let scatter_result = light_material.scatter(&ray_in, &rec, &mut fastrand::Rng::with_seed(0));

    assert!(
        scatter_result.is_none(),
        "El método scatter de DiffuseLight debe devolver None para detener los rebotes del rayo"
    );
}

#[test]
fn test_emitted_returns_correct_color() {
    // Validar que devuelva la radiancia constante sin importar los parámetros de entrada
    let expected_color = Color::new(12.5, 7.2, 3.0);
    let light_material = Material::DiffuseLight { emit: expected_color };

    // Probamos en el origen con coordenadas UV (0,0)
    let color_at_origin = light_material.emitted(0.0, 0.0, Point3::new(0.0, 0.0, 0.0));
    assert_color_near(color_at_origin, expected_color);

    // Probamos en una coordenada arbitraria del espacio con UVs en los extremos (1,1)
    let color_at_boundary = light_material.emitted(1.0, 1.0, Point3::new(-25.0, 4.2, 100.8));
    assert_color_near(color_at_boundary, expected_color);
}

#[test]
fn test_emitted_with_hdr_values() {
    // Las luces avanzadas usan valores de albedo mayores a 1.0 (HDR)
    // Este test asegura que el constructor no esté recortando (clamping) los valores a 1.0
    let hdr_color = Color::new(50.0, 50.0, 50.0);
    let light_material = Material::DiffuseLight { emit: hdr_color };

    let emitted_color = light_material.emitted(0.5, 0.5, Point3::new(1.0, 2.0, 3.0));

    assert!(
        emitted_color.x > 1.0 && emitted_color.y > 1.0 && emitted_color.z > 1.0,
        "El material debe soportar e irradiar intensidades de luz HDR mayores a 1.0"
    );
    assert_color_near(emitted_color, hdr_color);
}

// Función auxiliar para comparar vectores de color con tolerancia flotante
fn assert_color_near(a: Color, b: Color) {
    let epsilon = 1e-4;
    assert!(
        (a.x - b.x).abs() < epsilon && (a.y - b.y).abs() < epsilon && (a.z - b.z).abs() < epsilon,
        "Los colores no coinciden: {:?} vs {:?}",
        a,
        b
    );
}
