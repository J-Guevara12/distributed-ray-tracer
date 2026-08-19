use rt_core::{Point3, Ray, Vec3};

use crate::{Hittable, Interval, aabb::Aabb, bvh::Bvh, geometry::Sphere, primitive::Primitive};

fn sphere(x: f32, y: f32, z: f32, radius: f32) -> Primitive {
    Sphere::new(Point3::new(x, y, z), radius, 0).into()
}

// =========================================================================
// AABB
// =========================================================================

fn unit_box() -> Aabb {
    Aabb::from_points(Point3::splat(-1.0), Point3::splat(1.0))
}

#[test]
fn test_aabb_ray_intersection_hit() {
    let bbox = unit_box();
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
    let bbox = unit_box();
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
    let bbox = Aabb::from_points(Point3::splat(1.0), Point3::splat(3.0));
    let ray = Ray::new(Point3::new(2.0, 2.0, 0.0), Vec3::new(0.0, 0.0, 1.0));

    // El impacto físico ocurre en t = 1.0; el intervalo termina antes
    assert!(
        !bbox.hit(&ray, Interval::new(0.0, 0.5)),
        "La caja no debe impactar si ocurre fuera del rango de t permitido"
    );
}

#[test]
fn test_aabb_surrounding_box() {
    let box_a = Aabb::from_points(Point3::splat(-5.0), Point3::splat(-2.0));
    let box_b = Aabb::from_points(Point3::splat(1.0), Point3::splat(4.0));

    let big_box = Aabb::surrounding_box(box_a, box_b);

    assert_eq!(big_box.min, Point3::splat(-5.0));
    assert_eq!(big_box.max, Point3::splat(4.0));
}

/// `dir.z == 0` da `inv_dir.z = ±inf`, y si el origen cae justo sobre un plano
/// de la caja el producto `0 * inf` es NaN. Hoy falla: el slab branchless
/// descarta el rayo. Ver la nota de `Ray::new` sobre el clamp de `inv_dir`.
#[test]
#[ignore = "hueco conocido: rayo coplanar con una cara devuelve miss (NaN por 0 * inf)"]
fn test_aabb_ray_coplanar_with_face() {
    let bbox = Aabb::from_points(Point3::ZERO, Point3::splat(2.0));
    let t_range = Interval::new(0.001, f32::INFINITY);

    // Rayo contenido en el plano z = min.z, apuntando al interior de la cara.
    let coplanar = Ray::new(Point3::new(-5.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
    assert!(
        bbox.hit(&coplanar, t_range),
        "un rayo coplanar con la cara z = min.z se está descartando (NaN por 0 * inf)"
    );

    // Control: el mismo rayo desplazado apenas hacia adentro no depende del NaN.
    let interior = Ray::new(Point3::new(-5.0, 1.0, 0.5), Vec3::new(1.0, 0.0, 0.0));
    assert!(bbox.hit(&interior, t_range));
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
    assert_eq!(bvh.bounding_box().min.x, -1.0);
    assert_eq!(bvh.bounding_box().max.x, 1.0);
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
    assert_eq!(root.min.x, -10.0);
    assert_eq!(root.max.x, 10.0);
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

/// El invariante que de verdad importa: el BVH es una optimización, así que
/// tiene que devolver exactamente lo mismo que revisar todas las primitivas.
///
/// Con geometría no degenerada la igualdad es exacta, bit a bit. Donde deja de
/// serlo es sobre superficies coincidentes (dos quads compartiendo arista, por
/// ejemplo), y ahí no hay respuesta correcta: los dos están a la misma
/// distancia y gana el que se pruebe último. Por eso la escena de este test son
/// esferas sueltas — ver la nota de LEARNED_LESSONS sobre las aristas de B1.
#[test]
fn test_bvh_matches_linear_scan() {
    use crate::linear_scan::LinearScan;

    let mut rng = fastrand::Rng::with_seed(0xB0A7);
    let mut coord = |scale: f32| (rng.f32() - 0.5) * scale;

    // Genero los parámetros una vez y construyo las dos estructuras a partir de
    // ellos: `PlanarShape` no es `Clone`, y de todas formas el test necesita que
    // las dos vean exactamente la misma geometría.
    let specs: Vec<(Point3, f32, u32)> = (0..120)
        .map(|i| {
            let center = Point3::new(coord(40.0), coord(40.0), coord(40.0));
            (center, 0.5 + coord(1.5).abs(), (i % 4) as u32)
        })
        .collect();

    let primitives = |specs: &[(Point3, f32, u32)]| -> Vec<Primitive> {
        specs.iter().map(|(c, r, m)| Sphere::new(*c, *r, *m).into()).collect()
    };

    let bvh = Bvh::build(primitives(&specs));
    let linear = LinearScan::new(primitives(&specs));

    let range = Interval::new(0.001, f32::INFINITY);
    let mut checked = 0;

    for _ in 0..20_000 {
        let origin = Point3::new(coord(80.0), coord(80.0), coord(80.0));
        let direction = Vec3::new(coord(2.0), coord(2.0), coord(2.0));
        if direction.length_squared() < 1e-6 {
            continue;
        }

        let ray = Ray::new(origin, direction);
        let from_bvh = bvh.hit(&ray, range);
        let from_linear = linear.hit(&ray, range);

        match (&from_bvh, &from_linear) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert_eq!(a.t, b.t, "distinta distancia para el mismo rayo");
                assert_eq!(a.material, b.material, "distinta primitiva a la misma t");
                assert_eq!(a.normal, b.normal, "distinta normal");
            }
            _ => panic!(
                "solo una de las dos estructuras reportó impacto: bvh={:?} lineal={:?}",
                from_bvh.map(|r| r.t),
                from_linear.map(|r| r.t)
            ),
        }
        checked += 1;
    }

    assert!(checked > 15_000, "el test degeneró: solo {checked} rayos válidos");
}
