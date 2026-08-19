use rt_core::{Point3, Ray, Vec3};

use crate::{Hittable, Interval, aabb::Aabb, bvh::Bvh, geometry::Sphere, primitive::Primitive};

fn sphere(x: f32, y: f32, z: f32, radius: f32) -> Primitive {
    Sphere::new(Point3::new(x, y, z), radius, 0).into()
}

// =========================================================================
// AABB
// =========================================================================

#[test]
fn test_aabb_ray_intersection_hit() {
    let bbox = Aabb {
        x: Interval::new(-1.0, 1.0),
        y: Interval::new(-1.0, 1.0),
        z: Interval::new(-1.0, 1.0),
    };
    let t_range = Interval::new(0.001, f32::INFINITY);

    let ray_front = Ray::new(Point3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
    assert!(
        bbox.hit(&ray_front, t_range),
        "El rayo frontal debió impactar el centro de la caja"
    );

    let ray_diagonal = Ray::new(Point3::new(-2.0, -2.0, -2.0), Vec3::new(1.0, 1.0, 1.0));
    assert!(
        bbox.hit(&ray_diagonal, t_range),
        "El rayo diagonal debió cruzar la caja de esquina a esquina"
    );
}

#[test]
fn test_aabb_ray_intersection_miss() {
    let bbox = Aabb {
        x: Interval::new(-1.0, 1.0),
        y: Interval::new(-1.0, 1.0),
        z: Interval::new(-1.0, 1.0),
    };
    let t_range = Interval::new(0.001, f32::INFINITY);

    let paralelo = Ray::new(Point3::new(2.5, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
    assert!(
        !bbox.hit(&paralelo, t_range),
        "Un rayo paralelo por fuera no debe impactar la caja"
    );

    let alejandose = Ray::new(Point3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, 1.0));
    assert!(
        !bbox.hit(&alejandose, t_range),
        "Un rayo que se aleja de la caja no debe registrar impacto"
    );
}

#[test]
fn test_aabb_interval_constraints() {
    let bbox = Aabb {
        x: Interval::new(1.0, 3.0),
        y: Interval::new(1.0, 3.0),
        z: Interval::new(1.0, 3.0),
    };
    let ray = Ray::new(Point3::new(2.0, 2.0, 0.0), Vec3::new(0.0, 0.0, 1.0));

    // El impacto físico ocurre en t = 1.0; el intervalo termina antes
    assert!(
        !bbox.hit(&ray, Interval::new(0.0, 0.5)),
        "La caja no debe impactar si ocurre fuera del rango de t permitido"
    );
}

#[test]
fn test_aabb_surrounding_box() {
    let box_a = Aabb {
        x: Interval::new(-5.0, -2.0),
        y: Interval::new(-5.0, -2.0),
        z: Interval::new(-5.0, -2.0),
    };
    let box_b = Aabb {
        x: Interval::new(1.0, 4.0),
        y: Interval::new(1.0, 4.0),
        z: Interval::new(1.0, 4.0),
    };

    let big_box = Aabb::surrounding_box(box_a, box_b);

    assert_eq!(big_box.x.min, -5.0);
    assert_eq!(big_box.x.max, 4.0);
    assert_eq!(big_box.y.min, -5.0);
    assert_eq!(big_box.y.max, 4.0);
    assert_eq!(big_box.z.min, -5.0);
    assert_eq!(big_box.z.max, 4.0);
}

// =========================================================================
// BVH plano
// =========================================================================

#[test]
fn test_bvh_empty_never_hits() {
    let bvh = Bvh::build(vec![]);
    let ray = Ray::new(Point3::ZERO, Vec3::new(0.0, 0.0, -1.0));

    assert!(bvh.hit(&ray, Interval::new(0.001, f32::INFINITY)).is_none());
    assert_eq!(bvh.node_count(), 0);
}

#[test]
fn test_bvh_single_object_bounds() {
    let bvh = Bvh::build(vec![sphere(0.0, 0.0, 0.0, 1.0)]);

    assert_eq!(bvh.primitive_count(), 1);
    assert_eq!(bvh.bounding_box().x.min, -1.0);
    assert_eq!(bvh.bounding_box().x.max, 1.0);
}

#[test]
fn test_bvh_root_bounds_cover_every_primitive() {
    // Objetos separados a lo largo de X para forzar la división espacial
    let bvh = Bvh::build(vec![
        sphere(-9.0, 0.0, 0.0, 1.0),
        sphere(0.0, 0.0, 0.0, 1.0),
        sphere(9.0, 0.0, 0.0, 1.0),
    ]);

    let root = bvh.bounding_box();
    assert_eq!(root.x.min, -10.0);
    assert_eq!(root.x.max, 10.0);
}

#[test]
fn test_bvh_returns_closest_hit() {
    // Cercano en Z = -2, lejano en Z = -10. El recorrido debe devolver el
    // cercano sin importar en qué orden queden en el árbol.
    let bvh = Bvh::build(vec![
        sphere(0.0, 0.0, -10.0, 1.0),
        sphere(0.0, 0.0, -2.0, 1.0),
    ]);

    let ray = Ray::new(Point3::ZERO, Vec3::new(0.0, 0.0, -1.0));
    let rec = bvh
        .hit(&ray, Interval::new(0.001, f32::INFINITY))
        .expect("El BVH debió reportar un impacto");

    // La superficie de la esfera cercana está en z = -1
    assert!(
        (rec.t - 1.0).abs() < 1e-4,
        "El árbol devolvió un impacto lejano ocultando el más cercano (t = {})",
        rec.t
    );
}

#[test]
fn test_bvh_misses_when_ray_avoids_bounds() {
    let bvh = Bvh::build(vec![sphere(101.0, 101.0, 101.0, 1.0)]);

    // Rayo que se aleja del único objeto
    let ray = Ray::new(Point3::ZERO, Vec3::new(0.0, 0.0, -1.0));
    assert!(bvh.hit(&ray, Interval::new(0.001, f32::INFINITY)).is_none());
}

#[test]
fn test_bvh_leaves_hold_multiple_primitives() {
    // Con 4 primitivas o menos, todo cabe en una hoja: un solo nodo.
    let small = Bvh::build(vec![
        sphere(0.0, 0.0, 0.0, 0.5),
        sphere(2.0, 0.0, 0.0, 0.5),
        sphere(4.0, 0.0, 0.0, 0.5),
        sphere(6.0, 0.0, 0.0, 0.5),
    ]);
    assert_eq!(small.node_count(), 1, "4 primitivas caben en una hoja");

    // Con más, el árbol se parte y todas siguen alcanzables.
    let big: Vec<Primitive> = (0..17)
        .map(|i| sphere(i as f32 * 2.0, 0.0, 0.0, 0.5))
        .collect();
    let bvh = Bvh::build(big);
    assert!(bvh.node_count() > 1);
    assert_eq!(bvh.primitive_count(), 17);

    for i in 0..17 {
        let origin = Point3::new(i as f32 * 2.0, 5.0, 0.0);
        let ray = Ray::new(origin, Vec3::new(0.0, -1.0, 0.0));
        assert!(
            bvh.hit(&ray, Interval::new(0.001, f32::INFINITY)).is_some(),
            "la primitiva {i} quedó inalcanzable tras aplanar el árbol"
        );
    }
}
