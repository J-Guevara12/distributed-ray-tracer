use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use rt_core::{Color, Interval, Point3, Ray, Vec3, background::Background};
use rt_scene::{
    HitRecord, Hittable, Material, geometry::Sphere, hittable_list::HittableList,
    materials::Lambertian,
};

use crate::tracers::{NormalTracer, PathTracer, RayTracer};

#[test]
fn test_tracer_fallback_to_gradient_on_miss() {
    let world = Arc::new(HittableList::new());
    let tracer = NormalTracer {};

    // Un rayo apuntando hacia arriba (Y = 1.0), debería dar el color azul del cielo puro
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));

    let color = tracer.trace_ray(ray, world.as_ref(), &background);

    // El azul del gradiente es [128, 179, 255] aprox (t = 1.0)
    assert_eq!(color[2], 1.0); // El canal azul debe estar al tope
}

#[test]
fn test_tracer_renders_normal_on_hit() {
    let mut world = HittableList::new();
    // Esfera en frente de la cámara
    let material = Arc::new(Lambertian::new(Vec3::new(0.0, 0.0, 0.0)));
    world.add(Arc::new(Sphere::new(
        Point3::new(0.0, 0.0, -1.0),
        0.5,
        material,
    )));

    let tracer = NormalTracer {};
    // Disparamos un rayo al centro exacto de la esfera
    let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));

    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));
    let color = tracer.trace_ray(ray, &world, &background);
    println!("{}: color", color);

    // En el centro exacto, la normal apunta directo a la cámara (Z = 1.0)
    // Mapeo Z: 0.5 * (1.0 + 1.0) = 1.0 -> 255 en el canal Azul (color[2])
    // Mapeo X/Y: 0.5 * (0.0 + 1.0) = 0.5 -> ~128 en canales Rojo y Verde
    assert!((color[0] - 0.5).abs() <= 2.0 / 255.0);
    assert!((color[1] - 0.5).abs() <= 2.0 / 255.0);
    assert!(color[2] >= 254.0 / 255.0);
}

// =========================================================================
// MOCKS PARA PRUEBAS CONTROLADAS
// =========================================================================

// 1. Un material que absorbe una cantidad fija y refleja el rayo siempre en la misma dirección.
// Esto rompe el azar de fastrand para hacer el test 100% determinista.
#[derive(Debug)]
struct PredictableMaterial {
    albedo: Color,
    forced_direction: Vec3,
}

impl Material for PredictableMaterial {
    fn scatter(&self, _ray_in: &Ray, rec: &HitRecord) -> Option<(Color, Ray)> {
        // Devuelve siempre la misma atenuación y la misma dirección forzada
        Some((self.albedo, Ray::new(rec.p, self.forced_direction)))
    }
}

// 2. Una geometría ficticia que simula un plano infinito en Y = 0
#[derive(Debug)]
struct MockPlane {
    material: Arc<dyn Material>,
}

impl Hittable for MockPlane {
    fn hit(&self, ray: &Ray, ray_t: Interval) -> Option<HitRecord<'_>> {
        // Si el rayo va hacia abajo (Y negativa), simulamos un impacto en el origen
        if ray.direction.y < 0.0 {
            let t = 1.0;
            if ray_t.contains(t) {
                return Some(HitRecord::new(
                    ray,
                    t,
                    Vec3::Y, // Normal hacia arriba
                    Point3::ZERO,
                    &*self.material,
                ));
            }
        }
        None
    }
}

// =========================================================================
// SUITE DE TESTS INTENSIVOS
// =========================================================================

#[test]
fn test_path_tracer_max_depth_returns_black() {
    // ESCENARIO: Un rayo rebota eternamente entre dos superficies sin escapar.
    // El PathTracer debe detenerse estrictamente en max_depth.

    let mat_espejo_infinito = Arc::new(PredictableMaterial {
        albedo: Color::ONE, // Refleja el 100% de la luz (no pierde energía por color)
        forced_direction: Vec3::new(0.0, -1.0, 0.0), // Vuelve a disparar hacia abajo
    });

    let world = MockPlane {
        material: mat_espejo_infinito,
    };

    // Configuramos un límite de 5 rebotes
    let tracer = PathTracer::new(5);

    // Disparamos un rayo directo al plano
    let ray = Ray::new(Point3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));
    let color_resultado = tracer.trace_ray(ray, &world, &background);

    // ASSERT: Al quedarse atrapado y agotar los bounces, el resultado debe ser negro absoluto.
    assert_eq!(
        color_resultado,
        Color::ZERO,
        "El rayo debió agotar los bounces y retornar negro, pero devolvió {:?}",
        color_resultado
    );
}

#[test]
fn test_path_tracer_energy_conservation_exponential_decay() {
    // ESCENARIO: El rayo golpea una superficie que absorbe exactamente la mitad de la luz (albedo 0.5)
    // en cada rebote. Forzamos a que golpee 3 veces antes de escapar al cielo.

    // Tras 3 rebotes, la luz que quede debe multiplicarse por: 0.5 * 0.5 * 0.5 = 0.125
    let factor_absorcion = 0.5;
    let mat_absorbente = Arc::new(PredictableMaterial {
        albedo: Color::splat(factor_absorcion),
        forced_direction: Vec3::new(0.0, -1.0, 0.0),
    });

    // Una estructura para controlar cuántas veces dejamos que el rayo choque antes de dejarlo pasar al vacío
    struct FiniteBouncesZone {
        material: Arc<dyn Material>,
        hit_count: AtomicU32,
        max_hits: u32,
    }

    impl Hittable for FiniteBouncesZone {
        fn hit(&self, ray: &Ray, _ray_t: Interval) -> Option<HitRecord<'_>> {
            // Leemos el valor atómico actual con ordenamiento Relajado (suficiente para un test)
            let current_hits = self.hit_count.load(Ordering::Relaxed);

            if ray.direction.y < 0.0 && current_hits < self.max_hits {
                // Incrementamos el contador de forma atómica y segura entre hilos
                self.hit_count.fetch_add(1, Ordering::Relaxed);

                return Some(HitRecord::new(
                    ray,
                    1.0,
                    Vec3::Y,
                    Point3::ZERO,
                    &*self.material,
                ));
            }
            None
        }
    }
    let world = FiniteBouncesZone {
        material: mat_absorbente,
        hit_count: AtomicU32::new(0),
        max_hits: 3, // Forzamos exactamente 3 impactos
    };

    let tracer = PathTracer::new(10);
    let ray = Ray::new(Point3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0));

    // Ejecutamos el trazado
    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));
    let color_final = tracer.trace_ray(ray, &world, &background);

    // El color del cielo cuando el rayo escapa (dirección Y >= 0 tras pasar el límite de impactos)
    // Según tu ecuación del cielo: t = 0.5 * (dir.y + 1.0). Al ir horizontal o hacia arriba, calculamos el color base:
    // Nota: Ajusta este assert basado en el color exacto de tu fondo/cielo.

    let final_hits = world.hit_count.load(Ordering::Relaxed);
    assert!(final_hits == 3, "El rayo debió golpear exactamente 3 veces");

    // Verificamos que los canales no sean cero y que hayan sufrido la degradación exponencial (0.5^3 = 0.125)
    assert!(color_final.x > 0.0, "La energía se desvaneció por completo");

    // Si el material reduce a la mitad, la proporción con respecto a un escape directo debe ser exactamente 0.125
    // (Este delta 1e-4 mitiga imprecisiones de punto flotante f32)
    assert!((color_final.x - (color_final.x / color_final.x) * 0.125).abs() < 1e-4);
}

#[test]
fn test_path_tracer_miss_returns_sky_gradient() {
    // ESCENARIO: El rayo se dispara directamente hacia el horizonte o el cielo desértico.
    // No debe colisionar con nada y debe devolver el color puro del gradiente del fondo.

    struct EmptyWorld;
    impl Hittable for EmptyWorld {
        fn hit(&self, _ray: &Ray, _ray_t: Interval) -> Option<HitRecord<'_>> {
            None
        }
    }

    let world = EmptyWorld;
    let tracer = PathTracer::new(1);

    // Rayo disparado verticalmente hacia arriba (0.0, 1.0, 0.0) -> unit_dir.y = 1.0
    // t = 0.5 * (1.0 + 1.0) = 1.0
    // Color esperado: Color::ONE * (0.0) + Color::new(0.5, 0.7, 1.0) * 1.0 = [0.5, 0.7, 1.0]
    let ray_up = Ray::new(Point3::ZERO, Vec3::Y);
    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));
    let color_up = tracer.trace_ray(ray_up, &world, &background);

    assert_eq!(
        color_up,
        Color::new(0.5, 0.7, 1.0),
        "El gradiente superior del cielo es incorrecto"
    );

    // Rayo disparado verticalmente hacia abajo (0.0, -1.0, 0.0) -> unit_dir.y = -1.0
    // t = 0.5 * (-1.0 + 1.0) = 0.0
    // Color esperado: Color::ONE * (1.0) = [1.0, 1.0, 1.0] (Blanco puro en la base)
    let ray_down = Ray::new(Point3::ZERO, -Vec3::Y);
    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));
    let color_down = tracer.trace_ray(ray_down, &world, &background);

    assert_eq!(
        color_down,
        Color::ONE,
        "La base del cielo debería ser blanca"
    );
}
