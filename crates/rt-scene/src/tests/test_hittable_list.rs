use std::sync::Arc;
use rt_core::*;
use crate::{geometry::Sphere, hittable_list::HittableList, Hittable};

#[test]
fn test_hittable_list_finds_closest_object() {
    let mut world = HittableList::new();
    
    // Esfera lejana en Z = -2.0
    world.add(Arc::new(Sphere::new(Point3::new(0.0, 0.0, -2.0), 0.5)));
    // Esfera cercana en Z = -1.0 (Esta debería tapar a la otra)
    world.add(Arc::new(Sphere::new(Point3::new(0.0, 0.0, -1.0), 0.5)));

    // Un rayo que avanza directo por el eje Z negativo
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));
    let interval = Interval::new(0.001, f32::INFINITY);

    let hit = world.hit(&ray, interval);
    
    assert!(hit.is_some());
    let rec = hit.unwrap();
    // El impacto debe ocurrir en la esfera cercana (t = 0.5 para tocar la superficie en Z = -0.5)
    assert!((rec.t - 0.5).abs() < 1e-4);
}
