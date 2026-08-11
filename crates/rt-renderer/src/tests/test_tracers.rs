use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use rt_core::{Color, Interval, Point3, Ray, Vec3, background::Background};
use rt_scene::{
    HitRecord, Hittable, Material, geometry::Sphere, hittable_list::HittableList,
    materials::Lambertian, Aabb,
};

use crate::tracers::{NormalTracer, PathTracer, RayTracer};

// =========================================================================
// MOCKS PARA PRUEBAS CONTROLADAS CON SOPORTE DE AABB PARA BVH
// =========================================================================

// 1. Un material determinista que elimina la aleatoriedad de fastrand en las pruebas
#[derive(Debug)]
struct PredictableMaterial {
    albedo: Color,
    forced_direction: Vec3,
}

impl Material for PredictableMaterial {
    fn scatter(&self, _ray_in: &Ray, rec: &HitRecord) -> Option<(Color, Ray)> {
        Some((self.albedo, Ray::new(rec.p, self.forced_direction)))
    }
}

// 2. Un plano infinito en Y = 0 compatible con el Trait Hittable
#[derive(Debug)]
struct MockPlane {
    material: Arc<dyn Material>,
}

impl Hittable for MockPlane {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>> {
        if ray.direction.y < 0.0 {
            let t = 1.0;
            if ray_t.contains(t) {
                return Some(HitRecord::new(
                    ray,
                    t,
                    Vec3::Y,
                    Point3::ZERO,
                    &*self.material,
                ));
            }
        }
        None
    }

    fn bounding_box(&self) -> Aabb {
        // Un plano infinito usa una caja acotada por valores extremos acolchada
        Aabb {
            x: Interval::new(-f32::INFINITY, f32::INFINITY),
            y: Interval::new(-0.01, 0.01),
            z: Interval::new(-f32::INFINITY, f32::INFINITY),
        }
    }
}

// =========================================================================
// SUITE DE TESTS PRÁCTICOS Y DE TRAZADORES
// =========================================================================

#[test]
fn test_tracer_fallback_to_gradient_on_miss() {
    let world = Arc::new(HittableList::new());
    let tracer = NormalTracer {};

    // Rayo apuntando verticalmente hacia arriba (Y = 1.0)
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));

    let color = tracer.trace_ray(ray, world.as_ref(), &background);

    // En Y = 1.0 puro, el gradiente debe devolver exactamente el color superior [0.5, 0.7, 1.0]
    assert_eq!(color[2], 1.0);
}

#[test]
fn test_tracer_renders_normal_on_hit() {
    let mut world = HittableList::new();
    let material = Arc::new(Lambertian::new(Vec3::new(0.0, 0.0, 0.0)));
    world.add(Arc::new(Sphere::new(
        Point3::new(0.0, 0.0, -1.0),
        0.5,
        material,
    )));

    let tracer = NormalTracer {};
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));

    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));
    let color = tracer.trace_ray(ray, &world, &background);

    // En el centro exacto, la normal mapeada (N + 1) * 0.5 debe dar:
    // X=0 -> 0.5, Y=0 -> 0.5, Z=1 -> 1.0
    assert!((color[0] - 0.5).abs() <= 2.0 / 255.0);
    assert!((color[1] - 0.5).abs() <= 2.0 / 255.0);
    assert!(color[2] >= 254.0 / 255.0);
}

#[test]
fn test_path_tracer_max_depth_returns_black() {
    let mat_espejo_infinito = Arc::new(PredictableMaterial {
        albedo: Color::ONE,
        forced_direction: Vec3::new(0.0, -1.0, 0.0),
    });

    let world = MockPlane {
        material: mat_espejo_infinito,
    };

    let tracer = PathTracer::new(5);
    let ray = Ray::new(Point3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));
    
    let color_resultado = tracer.trace_ray(ray, &world, &background);

    assert_eq!(
        color_resultado,
        Color::ZERO,
        "El rayo debió agotar los bounces y retornar negro"
    );
}

#[test]
fn test_path_tracer_energy_conservation_exponential_decay() {
    let factor_absorcion = 0.5;
    
    // 🟢 El material mantiene la dirección hacia abajo (-Y) para forzar los rebrotes sucesivos
    let mat_absorbente = Arc::new(PredictableMaterial {
        albedo: Color::splat(factor_absorcion),
        forced_direction: Vec3::new(0.0, -1.0, 0.0), 
    });

    struct FiniteBouncesZone {
        material: Arc<dyn Material>,
        hit_count: AtomicU32,
        max_hits: u32,
    }

    impl Hittable for FiniteBouncesZone {
        fn hit(&self, ray: &Ray, _ray_t: Interval) -> Option<HitRecord<'_>> {
            let current_hits = self.hit_count.load(Ordering::Relaxed);

            // Si aún no alcanza el máximo de impactos, absorbe y refleja el rayo
            if ray.direction.y < 0.0 && current_hits < self.max_hits {
                self.hit_count.fetch_add(1, Ordering::Relaxed);

                return Some(HitRecord::new(
                    ray,
                    1.0,
                    Vec3::Y,
                    Point3::ZERO,
                    &*self.material,
                ));
            }
            // En el 4to intento, ya no colisiona y deja pasar el rayo al cielo
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

    let world = FiniteBouncesZone {
        material: mat_absorbente,
        hit_count: AtomicU32::new(0),
        max_hits: 3,
    };

    let tracer = PathTracer::new(10);
    // Disparamos el primer rayo hacia abajo
    let ray = Ray::new(Point3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
    
    // Un cielo con color uniforme para que el cálculo de color al escapar sea simple
    let sky_color = Color::new(0.5, 0.7, 1.0);
    let background = Background::new_gradient(sky_color, sky_color);

    let color_final = tracer.trace_ray(ray, &world, &background);

    let final_hits = world.hit_count.load(Ordering::Relaxed);
    assert_eq!(final_hits, 3, "El rayo debió golpear exactamente 3 veces");

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
        fn hit(&self, _ray: &Ray, _ray_t: Interval) -> Option<HitRecord<'_>> {
            None
        }
        fn bounding_box(&self) -> Aabb {
            Aabb::default()
        }
    }

    let world = EmptyWorld;
    let tracer = PathTracer::new(1);

    let ray_up = Ray::new(Point3::ZERO, Vec3::Y);
    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));
    let color_up = tracer.trace_ray(ray_up, &world, &background);

    assert_eq!(
        color_up,
        Color::new(0.5, 0.7, 1.0),
        "El gradiente superior del cielo es incorrecto"
    );

    let ray_down = Ray::new(Point3::ZERO, -Vec3::Y);
    let color_down = tracer.trace_ray(ray_down, &world, &background);

    assert_eq!(
        color_down,
        Color::ONE,
        "La base del cielo debería ser blanca"
    );
}
