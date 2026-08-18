use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use rt_core::{Color, Interval, Point3, Ray, Vec3, background::Background};
use rt_scene::{
    Aabb, HitRecord, Hittable, Material, Scene, geometry::Sphere, hittable_list::HittableList,
};

use crate::stats::RayStats;
use crate::tracers::{NormalTracer, PathTracer, RayContext, RayTracer};

fn ctx() -> RayContext {
    RayContext {
        rng: fastrand::Rng::with_seed(0),
        stats: RayStats::default(),
    }
}

fn scene(world: Arc<dyn Hittable>, materials: Vec<Material>, background: Background) -> Scene {
    Scene {
        world,
        materials,
        background,
    }
}

fn sky() -> Background {
    Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0))
}

/// Golpea siempre, sin importar la dirección: los materiales reales dispersan
/// aleatoriamente, así que un mock condicionado a `direction.y < 0` dejaría de
/// rebotar en cuanto el lambertiano apunte hacia arriba.
struct AlwaysHit {
    material: u32,
}

impl Hittable for AlwaysHit {
    fn hit(&self, ray: &Ray, _ray_t: Interval) -> Option<HitRecord> {
        Some(HitRecord::new(
            ray,
            1.0,
            Vec3::Y,
            Point3::ZERO,
            self.material,
        ))
    }

    fn bounding_box(&self) -> Aabb {
        Aabb {
            x: Interval::new(-1.0, 1.0),
            y: Interval::new(-1.0, 1.0),
            z: Interval::new(-1.0, 1.0),
        }
    }
}

#[test]
fn test_tracer_fallback_to_gradient_on_miss() {
    let world = Arc::new(HittableList::new());
    let tracer = NormalTracer {};

    // Rayo apuntando verticalmente hacia arriba (Y = 1.0)
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    let scene = scene(world, vec![], sky());

    let color = tracer.trace_ray(ray, &scene, &mut ctx());

    // En Y = 1.0 puro, el gradiente debe devolver el color superior
    assert_eq!(color[2], 1.0);
}

#[test]
fn test_tracer_renders_normal_on_hit() {
    let mut world = HittableList::new();
    world.add(Arc::new(Sphere::new(Point3::new(0.0, 0.0, -1.0), 0.5, 0)));

    let tracer = NormalTracer {};
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));

    let materials = vec![Material::Lambertian {
        albedo: Color::ZERO,
    }];
    let scene = scene(Arc::new(world), materials, sky());
    let color = tracer.trace_ray(ray, &scene, &mut ctx());

    // En el centro exacto, la normal mapeada (N + 1) * 0.5 debe dar:
    // X=0 -> 0.5, Y=0 -> 0.5, Z=1 -> 1.0
    assert!((color[0] - 0.5).abs() <= 2.0 / 255.0);
    assert!((color[1] - 0.5).abs() <= 2.0 / 255.0);
    assert!(color[2] >= 254.0 / 255.0);
}

#[test]
fn test_path_tracer_max_depth_returns_black() {
    // Mundo que siempre golpea con albedo 1.0: el camino nunca escapa al
    // fondo, así que debe agotar la profundidad y devolver negro.
    let world = Arc::new(AlwaysHit { material: 0 });
    let materials = vec![Material::Lambertian { albedo: Color::ONE }];
    let scene = scene(world, materials, sky());

    let tracer = PathTracer::new(5);
    let ray = Ray::new(Point3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0));

    let mut context = ctx();
    let color = tracer.trace_ray(ray, &scene, &mut context);

    assert_eq!(
        color,
        Color::ZERO,
        "El rayo debió agotar los bounces y retornar negro"
    );
    assert_eq!(
        context.stats.rays, 5,
        "un camino que siempre acierta traza un segmento por nivel de profundidad"
    );
}

#[test]
fn test_path_tracer_energy_conservation_exponential_decay() {
    struct FiniteBounces {
        hit_count: AtomicU32,
        max_hits: u32,
    }

    impl Hittable for FiniteBounces {
        fn hit(&self, ray: &Ray, _ray_t: Interval) -> Option<HitRecord> {
            if self.hit_count.load(Ordering::Relaxed) < self.max_hits {
                self.hit_count.fetch_add(1, Ordering::Relaxed);
                return Some(HitRecord::new(ray, 1.0, Vec3::Y, Point3::ZERO, 0));
            }
            // Agotados los impactos, el rayo escapa al cielo
            None
        }

        fn bounding_box(&self) -> Aabb {
            Aabb {
                x: Interval::new(-1.0, 1.0),
                y: Interval::new(-1.0, 1.0),
                z: Interval::new(-1.0, 1.0),
            }
        }
    }

    let world = Arc::new(FiniteBounces {
        hit_count: AtomicU32::new(0),
        max_hits: 3,
    });
    let counter = Arc::clone(&world);

    let factor_absorcion = 0.5;
    let materials = vec![Material::Lambertian {
        albedo: Color::splat(factor_absorcion),
    }];

    // Cielo uniforme para que el cálculo al escapar sea simple
    let sky_color = Color::new(0.5, 0.7, 1.0);
    let scene = scene(
        world,
        materials,
        Background::new_gradient(sky_color, sky_color),
    );

    let tracer = PathTracer::new(10);
    let ray = Ray::new(Point3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0));

    let mut context = ctx();
    let color_final = tracer.trace_ray(ray, &scene, &mut context);

    assert_eq!(
        counter.hit_count.load(Ordering::Relaxed),
        3,
        "El rayo debió golpear exactamente 3 veces"
    );
    assert_eq!(
        context.stats.rays, 4,
        "3 impactos más el segmento que escapa al fondo"
    );

    // Color esperado: sky_color * (0.5 ^ 3) = sky_color * 0.125
    let color_esperado = sky_color * 0.125;
    assert!((color_final.x - color_esperado.x).abs() < 1e-4);
    assert!((color_final.y - color_esperado.y).abs() < 1e-4);
    assert!((color_final.z - color_esperado.z).abs() < 1e-4);
}

#[test]
fn test_path_tracer_miss_returns_sky_gradient() {
    struct EmptyWorld;
    impl Hittable for EmptyWorld {
        fn hit(&self, _ray: &Ray, _ray_t: Interval) -> Option<HitRecord> {
            None
        }
        fn bounding_box(&self) -> Aabb {
            Aabb::default()
        }
    }

    let scene = scene(Arc::new(EmptyWorld), vec![], sky());
    let tracer = PathTracer::new(1);

    let color_up = tracer.trace_ray(Ray::new(Point3::ZERO, Vec3::Y), &scene, &mut ctx());
    assert_eq!(
        color_up,
        Color::new(0.5, 0.7, 1.0),
        "El gradiente superior del cielo es incorrecto"
    );

    let color_down = tracer.trace_ray(Ray::new(Point3::ZERO, -Vec3::Y), &scene, &mut ctx());
    assert_eq!(color_down, Color::ONE, "La base del cielo debería ser blanca");
}
