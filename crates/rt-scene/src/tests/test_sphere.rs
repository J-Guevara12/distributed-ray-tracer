use std::sync::Arc;

use rt_core::{Point3, Vec3};
use crate::{geometry::Sphere, *};

#[test]
fn test_sphere_hit_direct() {
    let sphere = Sphere::new(Point3::new(0.0, 0.0, -5.0), 1.0, 0);
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));
    let interval = Interval::new(0.0, 100.0);

    let hit = sphere.hit(&ray, interval);
    assert!(hit.is_some());
    
    let record = hit.unwrap();
    assert_eq!(record.t, 4.0); // Origen a 0, esfera en -5 con radio 1 => impacto en z = -4
    assert_eq!(record.p, Point3::new(0.0, 0.0, -4.0));
    assert_eq!(record.normal, Vec3::new(0.0, 0.0, 1.0)); // Normal mirando al rayo
    assert!(record.front_face);
}

#[test]
fn test_sphere_miss() {
    let sphere = Sphere::new(Point3::new(0.0, 5.0, -5.0), 1.0, 0); // Esfera movida hacia arriba
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));
    let interval = Interval::new(0.0, 100.0);

    let hit = sphere.hit(&ray, interval);
    assert!(hit.is_none());
}
