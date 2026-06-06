use rt_core::{Point3, Vec3};

use crate::camera::{Camera, CameraConfig};

fn setup_default_camera() -> Camera {
    let config = CameraConfig {
        aspect_ratio: 16.0 / 9.0,
        image_width: 400,
        fov: 90.0,
        look_from: Point3::new(0.0, 0.0, 0.0),
        look_at: Point3::new(0.0, 0.0, -1.0),
        vup: Vec3::new(0.0, 1.0, 0.0),
        samples_per_pixel: 10,
    };
    Camera::new(config)
}

#[test]
fn test_camera_center_ray_direction() {
    let camera = setup_default_camera();
    
    // El rayo del centro exacto de la pantalla (con sample 0 para evitar aleatoriedad)
    // Para una imagen de 400x225 (aprox 16:9), el centro está cerca de x=200, y=112
    let ray = camera.get_ray(200, 112, 0);
    
    // La dirección debe apuntar hacia el frente (Z negativo)
    assert!(ray.direction.z < 0.0, "El rayo debe apuntar hacia adelante");
    assert!(ray.direction.x.abs() < 0.05, "El rayo central no debería desviarse mucho en X");
}

#[test]
fn test_camera_ray_normalization() {
    let camera = setup_default_camera();
    
    // Esquinas de la pantalla
    let rays = vec![
        camera.get_ray(0, 0, 0),
        camera.get_ray(399, 0, 0),
        camera.get_ray(0, 224, 0),
        camera.get_ray(399, 224, 0),
    ];

    for ray in rays {
        let length = ray.direction.length();
        assert!((length - 1.0).abs() < 1e-5, "La dirección del rayo de la cámara debe estar normalizada: {}", length);
    }
}
