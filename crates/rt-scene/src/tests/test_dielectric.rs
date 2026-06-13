use crate::{HitRecord, Material, ScatterResult, materials::Dielectric};
use glam::Vec3A;
use rt_core::{Point3, Ray};

#[derive(Debug)]
struct MockMaterial;

impl Material for MockMaterial { fn scatter(&self, _: &Ray, _: &HitRecord, _: &mut fastrand::Rng) -> Option<ScatterResult> { None } }


/// Extrae (atenuación, rayo) de un rebote especular; falla si no lo es.
fn unwrap_specular(result: ScatterResult) -> (rt_core::Color, Ray) {
    match result {
        ScatterResult::Specular { attenuation, scattered } => (attenuation, scattered),
        other => panic!("Se esperaba un rebote especular, fue {:?}", other),
    }
}

#[test]
fn test_dielectric_perpendicular_incidence_does_not_bend() {
    let vidrio = Dielectric::new(1.5); // Índice de refracción típico del vidrio
    
    // Rayo que entra perfectamente vertical de arriba a abajo
    let ray_in = Ray::new(Point3::new(0.0, 2.0, 0.0), Vec3A::new(0.0, -1.0, 0.0));
    let mock_mat = MockMaterial;
    let rec = HitRecord {
        u: 0.0,
        v: 0.0,
        tangent: Vec3A::ZERO,
        p: Point3::ZERO,
        normal: Vec3A::Y, // Normal hacia arriba
        t: 2.0,
        front_face: true, // Viene desde afuera
        material: &mock_mat,
    };

    let (attenuation, ray_scattered) = unwrap_specular(vidrio.scatter(&ray_in, &rec, &mut fastrand::Rng::new()).unwrap());

    // 1. El vidrio transparente no debe teñir la luz (atenuación blanca pura)
    assert_eq!(attenuation, Vec3A::ONE);

    // 2. Al entrar perpendicular, no hay refracción angular, debe seguir recto hacia abajo (Y = -1.0)
    let dir = ray_scattered.direction.normalize();
    assert!((dir.x).abs() < 1e-5);
    assert!((dir.y + 1.0).abs() < 1e-5, "El rayo se desvió a pesar de entrar perpendicular");
}

#[test]
fn test_dielectric_total_internal_reflection_edge_case() {
    let vidrio = Dielectric::new(1.5);
    
    // Simulamos un rayo que YA ESTÁ DENTRO del vidrio (incidencia desde adentro)
    // Viaja hacia arriba a la derecha con un ángulo muy agudo/rasante
    let ray_in = Ray::new(Point3::new(-1.0, -0.1, 0.0), Vec3A::new(1.0, 0.1, 0.0).normalize());
    let mock_mat = MockMaterial;
    
    let rec = HitRecord {
        u: 0.0,
        v: 0.0,
        tangent: Vec3A::ZERO,
        p: Point3::ZERO,
        normal: Vec3A::Y, // La superficie está arriba de él
        t: 1.0,
        front_face: false, // 🔴 CLAVE: El rayo está adentro e intenta salir al aire
        material: &mock_mat,
    };

    let (_, ray_scattered) = unwrap_specular(vidrio.scatter(&ray_in, &rec, &mut fastrand::Rng::new()).unwrap());

    // Caso límite físico: En este ángulo, la Ley de Snell daría un seno de refracción > 1.0 (Imposible).
    // El motor debe forzar Reflexión Interna Total. El rayo debe rebotar hacia abajo (Y negativa).
    assert!(
        ray_scattered.direction.y < 0.0, 
        "El rayo debió sufrir Reflexión Interna Total y rebotar hacia adentro, pero escapó (Y = {})", 
        ray_scattered.direction.y
    );
}

#[test]
fn test_schlick_approximation_reflectance_limits() {
    // Test directo de la función interna de Schlick (si la expusiste o puedes evaluarla mediante scatter)
    // Si golpeas un vidrio en un ángulo ultra rasante (casi 90 grados respecto a la normal),
    // la probabilidad de reflejar debe acercarse a 1.0 de forma asintótica.
    let vidrio = Dielectric::new(1.5);
    
    let ray_in = Ray::new(Point3::new(-50.0, 0.001, 0.0), Vec3A::new(1.0, -0.00001, 0.0).normalize());
    let mock_mat = MockMaterial;
    let rec = HitRecord {
        u: 0.0,
        v: 0.0,
        tangent: Vec3A::ZERO,
        p: Point3::ZERO,
        normal: Vec3A::Y,
        t: 50.0,
        front_face: true,
        material: &mock_mat,
    };

    // Corremos un muestreo estadístico. A este ángulo, casi el 100% de los rayos deben ser REFLEJADOS (Y > 0)
    // y casi ninguno REFRACTADO (Y < 0).
    let mut reflejados = 0;
    let iteraciones = 200;
    
    for _ in 0..iteraciones {
        let (_, ray) = unwrap_specular(vidrio.scatter(&ray_in, &rec, &mut fastrand::Rng::new()).unwrap());
        if ray.direction.y > 0.0 {
            reflejados += 1;
        }
    }

    let tasa_reflexion = reflejados as f32 / iteraciones as f32;
    assert!(
        tasa_reflexion > 0.90, 
        "La aproximación de Schlick falló en ángulo rasante. Solo reflejó el {}%", 
        tasa_reflexion * 100.0
    );
}
