use crate::*;
use proptest::prelude::*;


#[test]
fn test_ray_at() {
    let origin = Point3::new(0.0, 0.0, 0.0);
    let direction = Vec3::new(0.0, 0.0, -1.0);

    let ray = Ray::new(origin, direction);
    assert_eq!(ray.at(0.0), Point3::new(0.0, 0.0, 0.0));
    assert_eq!(ray.at(2.5), Point3::new(0.0, 0.0, -2.5));
}

proptest! {
    #[test]
    fn test_direction_always_normalized(x in -100.0..100.0f32, y in -100.0..100.0f32, z in -100.0..100.0f32){
        if x.abs() > 1e-5 || y.abs() > 1e-5 || z.abs() > 1e-5 {
            let ray = Ray::new(Point3::ZERO, Vec3::new(x, y, z));
            let length = ray.direction.length();
            prop_assert!((length - 1.0).abs() < 1e-5, "La dirección no está normalizada: {}", length);
        }
    }
}
