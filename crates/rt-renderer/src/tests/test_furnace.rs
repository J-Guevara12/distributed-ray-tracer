//! White furnace test.
//!
//! A lambertian sphere lit by a uniform environment of radiance `L` reflects
//! exactly what it receives, so with albedo 1.0 it must become invisible: every
//! pixel equals `L` whether it hits the sphere or the background. Any deviation
//! is energy created or destroyed.
//!
//! It is the strongest correctness check an integrator has, and it is cheap
//! here: the sphere is convex, so every scattered ray escapes on the first
//! bounce and contributes exactly `albedo * L`. There is **no variance** — one
//! sample per pixel is enough and the comparison can be tight.
//!
//! Today it passes trivially, because the RTiOW lambertian has its cosine
//! factor implicit in `normal + random_unit_vector` and cannot get the
//! normalisation wrong. It earns its keep at F1.2, when BSDFs expose explicit
//! `sample()`/`pdf()` and a missing or duplicated factor becomes possible.

use std::sync::Arc;

use rt_core::background::Background;
use rt_core::{Color, Point3, Vec3, Vec4};
use rt_scene::geometry::Sphere;
use rt_scene::primitive::Primitive;
use rt_scene::{Material, Scene, bvh::Bvh};

use crate::camera::{Camera, CameraConfig};
use crate::framebuffer::FrameBuffer;
use crate::integrators::PathTracer;
use crate::render::render_scene;
use crate::tiles::TileResult;

/// Uniform environment radiance. Deliberately not 1.0 and not grey, so a
/// channel swap or a stray normalisation shows up.
const ENVIRONMENT: Color = Color::new(0.4, 0.7, 1.3);

const WIDTH: u32 = 64;
const SAMPLES: u32 = 4;

fn render(albedo: Color) -> Vec<Vec4> {
    let sphere: Primitive = Sphere::new(Point3::new(0.0, 0.0, -3.0), 1.0, 0).into();

    let scene = Scene {
        world: Arc::new(Bvh::build(vec![sphere])),
        materials: vec![Material::Lambertian { albedo }],
        background: Background::new_solid(ENVIRONMENT),
    };

    let camera = Camera::new(CameraConfig {
        aspect_ratio: 1.0,
        image_width: WIDTH,
        fov: 40.0,
        look_from: Point3::ZERO,
        look_at: Point3::new(0.0, 0.0, -1.0),
        vup: Vec3::new(0.0, 1.0, 0.0),
        samples_per_pixel: SAMPLES,
        defocus_angle: 0.0,
        focus_dist: 3.0,
    });

    let framebuffer = Arc::new(FrameBuffer::new(camera.width, camera.height));
    let snapshot = Arc::clone(&framebuffer);

    render_scene(
        Arc::new(camera),
        Arc::new(PathTracer::new(16)),
        framebuffer,
        &|_: &TileResult| {},
        16,
        &scene,
    );

    snapshot.get_snapshot()
}

/// Divides out the sample count and returns the per-pixel radiance.
fn resolved(image: &[Vec4]) -> Vec<Color> {
    image
        .iter()
        .map(|pixel| Color::new(pixel.x, pixel.y, pixel.z) / pixel.w.max(1.0))
        .collect()
}

fn assert_uniform(image: &[Vec4], expected: Color, tolerance: f32, what: &str) {
    let pixels = resolved(image);

    let worst = pixels
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            let error = |c: &Color| (*c - expected).abs().max_element();
            error(a).total_cmp(&error(b))
        })
        .expect("la imagen está vacía");

    let (index, value) = worst;
    let error = (*value - expected).abs().max_element();

    assert!(
        error <= tolerance,
        "{what}: el píxel {index} vale {value:?} y debería valer {expected:?} \
         (error {error:.6}, tolerancia {tolerance})"
    );
}

#[test]
fn test_furnace_albedo_one_makes_the_sphere_disappear() {
    let image = render(Color::ONE);
    assert_uniform(&image, ENVIRONMENT, 1e-5, "albedo 1.0");
}

/// El caso de albedo 1.0 por sí solo no alcanza: **1 es punto fijo de la
/// multiplicación**, así que un bug que aplique el albedo cero, una o tres veces
/// da el mismo resultado. Con 0.5 la esfera tiene que verse exactamente a la
/// mitad del fondo, y ahí sí se distingue.
#[test]
fn test_furnace_albedo_scales_the_sphere_exactly() {
    let albedo = Color::splat(0.5);
    let interior = ENVIRONMENT * albedo;
    let pixels = resolved(&render(albedo));

    // Los píxeles del borde de la silueta mezclan muestras que pegan a la
    // esfera con muestras que la pasan de largo, así que quedan entre los dos
    // valores. Lo que tiene que valer para TODOS es estar dentro del rango.
    for (index, value) in pixels.iter().enumerate() {
        for channel in 0..3 {
            let (low, high) = (interior[channel], ENVIRONMENT[channel]);
            assert!(
                value[channel] >= low - 1e-5 && value[channel] <= high + 1e-5,
                "el píxel {index} vale {value:?}, fuera del rango [{interior:?}, \
                 {ENVIRONMENT:?}]: hay energía creada o destruida"
            );
        }
    }

    let count = |target: Color| {
        pixels
            .iter()
            .filter(|pixel| (**pixel - target).abs().max_element() <= 1e-5)
            .count()
    };

    assert!(
        count(interior) > 0,
        "ningún píxel vale exactamente {interior:?}; la esfera no se está \
         encuadrando o el albedo no se aplica una sola vez"
    );
    assert!(
        count(ENVIRONMENT) > 0,
        "ningún píxel vale exactamente el fondo; la esfera tapa toda la imagen"
    );
}

/// Sin esto, un renderer que devolviera el fondo para todo píxel pasaría el
/// test de albedo 1.0 sin trazar una sola intersección.
#[test]
fn test_furnace_scene_actually_contains_the_sphere() {
    let pixels = resolved(&render(Color::ZERO));

    let dark = pixels
        .iter()
        .filter(|pixel| pixel.max_element() < 1e-5)
        .count();

    assert!(
        dark > 0,
        "con albedo 0.0 la esfera tiene que salir negra; no se está \
         intersectando nada"
    );
}
