use std::sync::Arc;

use rt_core::{Interval, Point3, Ray, Vec3};

use crate::{
    Bvh, Hittable,
    geometry::{Primitive, Quad, Sphere},
    hittable_list::HittableList,
    materials::Lambertian,
};

/// Genera una escena pseudoaleatoria reproducible de esferas y quads.
fn random_primitives(rng: &mut fastrand::Rng, n_objects: usize) -> Vec<Primitive> {
    let material = Arc::new(Lambertian::new(Vec3::splat(0.5)));
    let mut primitives = Vec::with_capacity(n_objects);

    for i in 0..n_objects {
        let center = Point3::new(
            rng.f32() * 20.0 - 10.0,
            rng.f32() * 20.0 - 10.0,
            rng.f32() * 20.0 - 10.0,
        );
        if i % 4 == 0 {
            let u = Vec3::new(rng.f32() + 0.1, 0.0, rng.f32());
            let v = Vec3::new(0.0, rng.f32() + 0.1, rng.f32());
            primitives.push(Primitive::Quad(Quad::new(center, u, v, Arc::clone(&material) as _)));
        } else {
            let radius = rng.f32() * 0.9 + 0.1;
            primitives.push(Primitive::Sphere(Sphere::new(center, radius, Arc::clone(&material) as _)));
        }
    }

    primitives
}

#[test]
fn test_bvh_matches_linear_scan() {
    let mut rng = fastrand::Rng::with_seed(42);
    let primitives = random_primitives(&mut rng, 200);

    let mut world = HittableList::new();
    for primitive in &primitives {
        world.add(Arc::new(primitive.clone()));
    }
    let bvh = Bvh::new(primitives);

    let interval = Interval::new(0.001, f32::INFINITY);

    for _ in 0..2000 {
        let origin = Point3::new(
            rng.f32() * 30.0 - 15.0,
            rng.f32() * 30.0 - 15.0,
            rng.f32() * 30.0 - 15.0,
        );
        let direction = Vec3::new(
            rng.f32() * 2.0 - 1.0,
            rng.f32() * 2.0 - 1.0,
            rng.f32() * 2.0 - 1.0,
        );
        if direction.length_squared() < 1e-6 {
            continue;
        }
        let ray = Ray::new(origin, direction);

        let linear_hit = world.hit(&ray, interval);
        let bvh_hit = bvh.hit(&ray, interval);

        match (linear_hit, bvh_hit) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert!(
                    (a.t - b.t).abs() < 1e-5,
                    "El BVH encontró una colisión distinta: t lineal = {}, t bvh = {}",
                    a.t,
                    b.t
                );
                assert_eq!(a.p, b.p, "Punto de impacto distinto");
                assert_eq!(a.normal, b.normal, "Normal distinta");
            }
            (a, b) => panic!(
                "Discrepancia de colisión: lineal = {:?}, bvh = {:?}",
                a.map(|r| r.t),
                b.map(|r| r.t)
            ),
        }
    }
}

#[test]
fn test_bvh_empty_world_returns_none() {
    let bvh = Bvh::new(Vec::new());
    let ray = Ray::new(Point3::ZERO, Vec3::Z);

    assert!(bvh.hit(&ray, Interval::new(0.001, f32::INFINITY)).is_none());
}

#[test]
fn test_bvh_single_object() {
    let material = Arc::new(Lambertian::new(Vec3::splat(0.5)));
    let sphere = Primitive::Sphere(Sphere::new(Point3::new(0.0, 0.0, -5.0), 1.0, material));
    let bvh = Bvh::new(vec![sphere]);

    let hit_ray = Ray::new(Point3::ZERO, -Vec3::Z);
    let miss_ray = Ray::new(Point3::ZERO, Vec3::Z);
    let interval = Interval::new(0.001, f32::INFINITY);

    let rec = bvh.hit(&hit_ray, interval).expect("Debe golpear la esfera");
    assert!((rec.t - 4.0).abs() < 1e-5);
    assert!(bvh.hit(&miss_ray, interval).is_none());
}

#[test]
fn test_bvh_respects_interval_max() {
    let material = Arc::new(Lambertian::new(Vec3::splat(0.5)));
    let sphere = Primitive::Sphere(Sphere::new(Point3::new(0.0, 0.0, -5.0), 1.0, material));
    let bvh = Bvh::new(vec![sphere]);

    let ray = Ray::new(Point3::ZERO, -Vec3::Z);
    // La esfera está a t = 4.0; con max = 2.0 no debe reportarse impacto
    assert!(bvh.hit(&ray, Interval::new(0.001, 2.0)).is_none());
}
