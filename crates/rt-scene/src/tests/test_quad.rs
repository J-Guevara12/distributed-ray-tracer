use std::sync::Arc;

use rt_core::{Color, Point3, Ray, Vec3};

use crate::{
    HitRecord, Hittable, Interval, Material,
    geometry::{PlanarShape, PlanarType},
};

// Función auxiliar para construir un cuadrilátero de prueba frente a la cámara.
// Es un cuadrado perfecto de 2x2 en el plano XY, empujado a Z = -2.0.
fn setup_test_quad() -> PlanarShape {
    let material = 0;
    PlanarShape::new(
        Point3::new(-1.0, -1.0, -2.0), // q: Esquina inferior izquierda
        Vec3::new(2.0, 0.0, 0.0),      // u: Se extiende 2 unidades a la derecha (Eje X)
        Vec3::new(0.0, 2.0, 0.0),      // v: Se extiende 2 unidades hacia arriba (Eje Y)
        PlanarType::Quad,
        material,
    )
}

#[test]
fn test_quad_hit_exact_center() {
    let quad = setup_test_quad();
    // Rayo en el origen apuntando al centro exacto del cuadrado (0.0, 0.0, -2.0)
    let ray = Ray {
        origin: Point3::new(0.0, 0.0, 0.0),
        direction: Vec3::new(0.0, 0.0, -1.0),
    };
    let interval = Interval::new(0.001, 10.0);

    let hit = quad.hit(&ray, interval);
    assert!(
        hit.is_some(),
        "El rayo debió impactar el centro del cuadrilátero"
    );

    let rec = hit.unwrap();
    assert_f32_near(rec.t, 2.0);
    assert_vec3_near(rec.p, Point3::new(0.0, 0.0, -2.0));

    // La normal debe apuntar hacia la cámara (Z positivo, por la regla de la mano derecha en U x V)
    assert_vec3_near(rec.normal, Vec3::new(0.0, 0.0, 1.0));
}

#[test]
fn test_quad_miss_outside_bounds() {
    let quad = setup_test_quad();
    // El rayo apunta a Z = -2.0 pero se pasa de largo por la derecha (X = 1.5, fuera del rango [-1, 1])
    let ray = Ray {
        origin: Point3::new(1.5, 0.0, 0.0),
        direction: Vec3::new(0.0, 0.0, -1.0),
    };
    let interval = Interval::new(0.001, 10.0);

    let hit = quad.hit(&ray, interval);
    assert!(
        hit.is_none(),
        "El rayo debió fallar porque impacta el plano pero fuera del cuadrilátero"
    );
}

#[test]
fn test_quad_hit_edges_and_corners() {
    let quad = setup_test_quad();
    let interval = Interval::new(0.001, 10.0);

    // Impacto directo en la esquina origen 'q' (-1.0, -1.0, -2.0)
    let ray_q = Ray {
        origin: Point3::new(-1.0, -1.0, 0.0),
        direction: Vec3::new(0.0, 0.0, -1.0),
    };
    assert!(
        quad.hit(&ray_q, interval).is_some(),
        "Debe registrar impacto en el vértice Q"
    );

    // Impacto en la esquina opuesta diagonal (q + u + v) -> (1.0, 1.0, -2.0)
    let ray_opposite = Ray {
        origin: Point3::new(1.0, 1.0, 0.0),
        direction: Vec3::new(0.0, 0.0, -1.0),
    };
    assert!(
        quad.hit(&ray_opposite, interval).is_some(),
        "Debe registrar impacto en la esquina superior derecha"
    );
}

#[test]
fn test_quad_miss_parallel_ray() {
    let quad = setup_test_quad();
    // Un rayo que viaja de izquierda a derecha a través del eje X. Es paralelo al plano del Quad.
    let ray = Ray {
        origin: Point3::new(-5.0, 0.0, -2.0),
        direction: Vec3::new(1.0, 0.0, 0.0),
    };
    let interval = Interval::new(0.001, 10.0);

    let hit = quad.hit(&ray, interval);
    assert!(
        hit.is_none(),
        "Los rayos paralelos al plano no deben colisionar"
    );
}

#[test]
fn test_quad_miss_behind_ray() {
    let quad = setup_test_quad();
    // El rayo apunta hacia Z positivo (atrás), dándole la espalda al objeto
    let ray = Ray {
        origin: Point3::new(0.0, 0.0, 0.0),
        direction: Vec3::new(0.0, 0.0, 1.0),
    };
    let interval = Interval::new(0.001, 10.0);

    let hit = quad.hit(&ray, interval);
    assert!(
        hit.is_none(),
        "No debe haber colisión si el objeto está detrás del rayo"
    );
}

#[test]
fn test_quad_out_of_interval_range() {
    let quad = setup_test_quad();
    let ray = Ray {
        origin: Point3::new(0.0, 0.0, 0.0),
        direction: Vec3::new(0.0, 0.0, -1.0),
    };
    // El impacto real requiere t = 2.0. Si acortamos el intervalo a 1.5, debe ignorarse.
    let interval_short = Interval::new(0.001, 1.5);

    let hit = quad.hit(&ray, interval_short);
    assert!(
        hit.is_none(),
        "Debe retornar None porque el impacto está más lejos del t_max permitido"
    );
}

// Funciones auxiliares para comparar flotantes y vectores con un margen de tolerancia (epsilon)
fn assert_f32_near(a: f32, b: f32) {
    assert!(
        (a - b).abs() < 1e-4,
        "Flotantes no coinciden: {} vs {}",
        a,
        b
    );
}

fn assert_vec3_near(a: Vec3, b: Vec3) {
    assert!(
        (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4 && (a.z - b.z).abs() < 1e-4,
        "Vectores difieren demasiado: {:?} vs {:?}",
        a,
        b
    );
}
