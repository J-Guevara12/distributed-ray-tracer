use crate::camera::{Camera, CameraConfig};
use crate::framebuffer::FrameBuffer;
use crate::render::render_scene;
use crate::tracers::RayTracer;
use rt_core::background::Background;
use rt_core::{Color, Point3, Ray, Vec3};
use rt_scene::Hittable;
use rt_scene::hittable_list::HittableList;
use std::sync::Arc;
use tokio::sync::broadcast;

/// 1. Creamos un "Mock" del RayTracer para las pruebas unitarias.
///    Su único trabajo es devolver un color fijo y conocido sin hacer cálculos complejos.
struct MockRayTracer {
    fixed_color: Color,
}

impl RayTracer for MockRayTracer {
    fn trace_ray(&self, _ray: Ray, _world: &dyn Hittable, _background: &Background) -> Color {
        // No importa la dirección del rayo, siempre devolvemos el mismo color
        self.fixed_color
    }
}

#[test]
fn test_render_scene_integration() {
    // 2. CONFIGURACIÓN: Definimos una resolución de pantalla muy pequeña (4x4)
    // para que el test sea ultra rápido y fácil de verificar byte por byte.
    let width = 4;
    let height = 4;
    let stride = 3; // RGB
    let tile_size = 2; // Partirá la pantalla en 4 tiles de 2x2

    // Instanciamos la cámara apuntando al frente
    let camera_config = CameraConfig {
        aspect_ratio: 1.0, // Pantalla cuadrada 4x4
        image_width: width,
        fov: 90.0,
        look_from: Point3::new(0.0, 0.0, 0.0),
        look_at: Point3::new(0.0, 0.0, -1.0),
        vup: Vec3::new(0.0, 1.0, 0.0),
        samples_per_pixel: 4, // Forzamos 4 muestras por píxel para probar el Antialiasing
        defocus_angle: 0.0,
        focus_dist: 1.0,
    };
    let camera = Arc::new(Camera::new(camera_config));

    // Instanciamos nuestro tracer simulado para que pinte todo de VERDE PURO
    let tracer = Arc::new(MockRayTracer {
        fixed_color: Color::new(0.0, 50.0, 0.0),
    });

    // Instanciamos el FrameBuffer y el canal de comunicación
    let framebuffer = Arc::new(FrameBuffer::new(width, height, stride));
    let (tx_stream, mut rx_stream) = broadcast::channel(10);
    let world = Arc::new(HittableList::new());
    let background = Background::new_gradient(Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));

    // 3. EJECUCIÓN: Corremos el orquestador de la escena
    render_scene(
        camera,
        tracer,
        Arc::clone(&framebuffer),
        tx_stream,
        tile_size,
        stride,
        world.as_ref(),
        &background,
    );

    // 4. ASERCIÓN 1: Validar el canal de comunicación (Broadcast)
    // Como la pantalla es de 4x4 y el tile_size es 2, el generador debió enviar exactamente 4 eventos.
    for i in 0..4 {
        let receive_result = rx_stream.try_recv();
        assert!(
            receive_result.is_ok(),
            "Se debió recibir el TileResult número {}",
            i
        );

        let tile_result = receive_result.unwrap();

        // Cada tile es de 2x2 píxeles * 3 bytes (RGB) = 12 bytes
        assert_eq!(
            tile_result.pixels.len(),
            12,
            "El tamaño del vector de píxeles del tile es incorrecto"
        );

        // Verificar que el primer píxel del tile sea el verde que inyectó el Mock
        assert_eq!(tile_result.pixels[0], 0);
        assert_eq!(tile_result.pixels[1], 255);
        assert_eq!(tile_result.pixels[2], 0);
    }

    // El canal ya debería estar vacío después de los 4 tiles
    assert!(
        rx_stream.try_recv().is_err(),
        "No deberían haber más de 4 tiles en el canal"
    );

    // 5. ASERCIÓN 2: Validar el estado final del FrameBuffer
    // Tomamos una captura de memoria del buffer completo
    let snapshot = framebuffer.get_snapshot();
    let expected_total_bytes = (width * height) as usize * stride; // 4 * 4 * 3 = 48 bytes
    assert_eq!(snapshot.len(), expected_total_bytes);

    // Validar que TODOS los píxeles de la imagen se hayan pintado de verde correctamente
    // a través del mapeo de filas de `write_tile` dentro de `render_scene`.
    for chunk in snapshot.chunks_exact(stride) {
        assert_eq!(chunk[0], 0, "El canal R debe ser 0");
        assert_eq!(chunk[1], 255, "El canal G debe ser 255");
        assert_eq!(chunk[2], 0, "El canal B debe ser 0");
    }
}
