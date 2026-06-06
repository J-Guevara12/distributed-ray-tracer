use rt_core::{Point3, Ray, Vec3};
use fastrand::Rng;

#[derive(Debug, Clone, Copy)]
pub struct CameraConfig {
    pub aspect_ratio: f32,
    pub image_width: u32,
    pub fov: f32,
    pub look_from: Point3,
    pub look_at: Point3,
    pub vup: Vec3,
    pub samples_per_pixel: u32,
}

#[derive(Debug, Clone)]
pub struct Camera {
    pub origin: Point3,
    pub pixel00_loc: Point3,    // Ubicación del píxel (0,0) en el espacio 3D
    pub pixel_delta_u: Vec3,    // Desplazamiento hacia la derecha entre píxeles
    pub pixel_delta_v: Vec3,    // Desplazamiento hacia abajo entre píxeles
    pub samples_per_pixel: u32, // Para el cálculo de Antialiasing
    pub width: u32,
    pub height: u32,
}

impl Camera {
    // Inicializa y calcula la geometría del Viewport basado en la configuración.
    pub fn new(config: CameraConfig) -> Self {
        let origin  = config.look_from;
        let samples_per_pixel = config.samples_per_pixel;

        let width = config.image_width;
        let height = ( width as f32 / config.aspect_ratio) as u32;

        let viewport_height = 2.0*(config.fov.to_radians()/2.0).tan();
        let viewport_width = viewport_height * config.aspect_ratio;

        let w = (origin - config.look_at).normalize();
        let u = (config.vup.cross(w)).normalize(); // Vector que apunta hacia la derecha de la cámara.
        let v = u.cross(w).normalize();     //Vector que apunta hacia abajo de la cámara

        let pixel_delta_u = (viewport_width/width as f32)*u;
        let pixel_delta_v = (viewport_height/height as f32)*v;


        let viewport_upper_left = origin - w - (u * viewport_width * 0.5) - (v * viewport_height * 0.5);
        let pixel00_loc = viewport_upper_left + 0.5 * (pixel_delta_u + pixel_delta_v);

        Self { origin, pixel00_loc, pixel_delta_u, pixel_delta_v, samples_per_pixel , width, height}
    }

    /// Genera un rayo dirigido al píxel (x, y). 
    /// Si `sample > 0`, aplica un desfase aleatorio sub-píxel (Antialiasing).
    pub fn get_ray(&self, x: u32, y: u32, sample: u32) -> Ray {
        debug_assert!(x < self.width, "get_ray: x ({}) must be less than the camera width ({})", x, self.width);
        debug_assert!(y < self.height, "get_ray: y ({}) must be less than the camera height ({})", y, self.height);
        let origin = self.origin;

        let offset = if sample == 0 {
            Vec3::new(0.0, 0.0, 0.0)
        } else {
            self.sample_square()
        };

        let  destination = self.pixel00_loc 
            + (x as f32+offset.x) * self.pixel_delta_u 
            + (y as f32 + offset.y) * self.pixel_delta_v;

        let direction = (destination-origin).normalize();

        Ray { origin, direction }
    }

    fn sample_square(&self) -> Vec3 {
        let mut rng = Rng::new();

        let rand_x = rng.f32() - 0.5;
        let rand_y = rng.f32() - 0.5;

        Vec3::new(rand_x, rand_y, 0.0)

    }
}
