use crate::camera::{Camera, CameraConfig};
use crate::framebuffer::FrameBuffer;
use crate::render::render_scene;
use crate::stats::RayStats;
use crate::tiles::TileResult;
use crate::tracers::RayTracer;
use rt_core::background::Background;
use rt_core::{Color, Point3, Ray, Vec3, Vec4};
use rt_scene::Hittable;
use rt_scene::hittable_list::HittableList;
use std::sync::{Arc, Mutex};

struct MockRayTracer {
    fixed_color: Color,
}

impl RayTracer for MockRayTracer {
    fn trace_ray(
        &self,
        _ray: Ray,
        _world: &dyn Hittable,
        _background: &Background,
        stats: &mut RayStats,
    ) -> Color {
        stats.rays += 1;
        self.fixed_color
    }
}

#[test]
fn test_render_scene_integration() {
    let width = 4;
    let height = 4;
    let tile_size = 2; // Parte la pantalla en 4 tiles de 2x2
    let samples = 4;

    let camera_config = CameraConfig {
        aspect_ratio: 1.0,
        image_width: width,
        fov: 90.0,
        look_from: Point3::new(0.0, 0.0, 0.0),
        look_at: Point3::new(0.0, 0.0, -1.0),
        vup: Vec3::new(0.0, 1.0, 0.0),
        samples_per_pixel: samples,
        defocus_angle: 0.0,
        focus_dist: 1.0,
    };
    let camera = Arc::new(Camera::new(camera_config));

    let tracer = Arc::new(MockRayTracer {
        fixed_color: Color::new(0.0, 50.0, 0.0),
    });

    let framebuffer = Arc::new(FrameBuffer::new(width, height));
    let world = Arc::new(HittableList::new());
    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));

    let collected: Mutex<Vec<TileResult>> = Mutex::new(Vec::new());
    let on_tile = |tile: &TileResult| collected.lock().unwrap().push(tile.clone());

    let stats = render_scene(
        camera,
        tracer,
        Arc::clone(&framebuffer),
        &on_tile,
        tile_size,
        world.as_ref(),
        &background,
    );

    // Un rayo por muestra por píxel, y un tiempo medido por tile.
    assert_eq!(stats.rays, (width * height * samples) as u64);
    assert_eq!(stats.tile_ms.len(), 4);

    // El sumidero debe recibir exactamente un tile por bloque de 2x2.
    let tiles = collected.into_inner().unwrap();
    assert_eq!(tiles.len(), 4, "se esperaba un TileResult por tile");

    // El framebuffer guarda la SUMA de radiancia, no el promedio: 4 muestras
    // del color del mock, y w = número de muestras.
    let expected = Vec4::new(0.0, 50.0 * samples as f32, 0.0, samples as f32);

    for tile in &tiles {
        assert_eq!(tile.pixels.len(), 4, "cada tile de 2x2 tiene 4 píxeles");
        assert_eq!(tile.pixels[0], expected);
    }

    let snapshot = framebuffer.get_snapshot();
    assert_eq!(snapshot.len(), (width * height) as usize);

    for pixel in snapshot {
        assert_eq!(pixel, expected);
    }
}
