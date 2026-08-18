use std::sync::Arc;

use rt_core::background::Background;
use rt_core::{Color, Point3, Vec3, Vec4};
use rt_scene::bvh::BvhNode;
use rt_scene::geometry::Sphere;
use rt_scene::hittable_list::HittableList;
use rt_scene::{Material, Scene};

use crate::camera::{Camera, CameraConfig};
use crate::framebuffer::FrameBuffer;
use crate::render::render_scene;
use crate::tiles::TileResult;
use crate::tracers::PathTracer;

/// Escena mínima que ejercita los cuatro sitios que consumen aleatoriedad:
/// jitter de píxel, disco de desenfoque, `random_unit_vector` (lambertiano y
/// fuzz de metal) y el volado de Schlick del dieléctrico.
fn scene() -> Scene {
    let mut list = HittableList::new();
    list.add(Arc::new(Sphere::new(
        Point3::new(0.0, -100.5, -1.0),
        100.0,
        0,
    )));
    list.add(Arc::new(Sphere::new(
        Point3::new(0.0, 0.0, -1.2),
        0.5,
        1,
    )));
    list.add(Arc::new(Sphere::new(
        Point3::new(-1.0, 0.0, -1.0),
        0.5,
        2,
    )));
    list.add(Arc::new(Sphere::new(
        Point3::new(1.0, 0.0, -1.0),
        0.5,
        3,
    )));

    Scene {
        world: BvhNode::build(list.objects),
        materials: vec![
            Material::Lambertian { albedo: Color::new(0.8, 0.8, 0.0) },
            Material::Lambertian { albedo: Color::new(0.1, 0.2, 0.5) },
            Material::Dielectric { refraction_index: 1.5 },
            Material::Metal { albedo: Color::new(0.8, 0.6, 0.2), fuzz: 0.3 },
        ],
        background: Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0)),
    }
}

fn render(threads: usize) -> Vec<Vec4> {
    let config = CameraConfig {
        aspect_ratio: 1.0,
        image_width: 64,
        fov: 40.0,
        look_from: Point3::new(0.0, 0.0, 1.0),
        look_at: Point3::new(0.0, 0.0, -1.0),
        vup: Vec3::new(0.0, 1.0, 0.0),
        samples_per_pixel: 4,
        defocus_angle: 2.0,
        focus_dist: 2.0,
    };

    let camera = Camera::new(config);
    let framebuffer = Arc::new(FrameBuffer::new(camera.width, camera.height));
    let scene = scene();
    let on_tile = |_: &TileResult| {};

    // Pool con alcance local: `RAYON_NUM_THREADS` solo se lee al inicializar el
    // pool global, una vez por proceso, así que no sirve dentro de `cargo test`.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("no se pudo crear el pool de rayon");

    pool.install(|| {
        render_scene(
            Arc::new(camera),
            Arc::new(PathTracer::new(8)),
            Arc::clone(&framebuffer),
            &on_tile,
            16,
            &scene,
        );
    });

    framebuffer.get_snapshot()
}

#[test]
fn test_render_is_independent_of_thread_count() {
    let single = render(1);
    let many = render(8);

    assert_eq!(single.len(), many.len());
    for (index, (a, b)) in single.iter().zip(many.iter()).enumerate() {
        assert_eq!(
            a, b,
            "el píxel {index} difiere entre 1 y 8 hilos: {a:?} vs {b:?}"
        );
    }
}

#[test]
fn test_render_is_reproducible() {
    assert_eq!(render(4), render(4));
}

/// Sin esto, un renderer que devolviera negro constante pasaría los dos tests
/// de arriba: hay que comprobar que la imagen tiene contenido y varía.
#[test]
fn test_render_output_is_non_trivial() {
    let image = render(4);
    let first = image[0];

    assert!(
        image.iter().any(|p| p.w > 0.0),
        "ningún píxel acumuló muestras"
    );
    assert!(
        image.iter().any(|p| *p != first),
        "todos los píxeles son idénticos; la escena no se está renderizando"
    );
}
