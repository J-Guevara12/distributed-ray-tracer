use crate::{Point3, Ray, Vec3};
use fastrand::Rng;
use optional_struct::*;
use serde::{Deserialize, Serialize};

pub use optional_struct::Applicable;

#[optional_struct(CameraUpdatePayload)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CameraConfig {
    pub aspect_ratio: f32,
    pub image_width: u32,
    pub fov: f32,
    pub look_from: Point3,
    pub look_at: Point3,
    pub vup: Vec3,
    pub samples_per_pixel: u32,
    pub defocus_angle: f32,
    pub focus_dist: f32,
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
    pub defocus_disk_u: Vec3,
    pub defocus_disk_v: Vec3,
    pub config: CameraConfig,
}

impl Camera {
    // Inicializa y calcula la geometría del Viewport basado en la configuración.
    pub fn new(config: CameraConfig) -> Self {
        let origin = config.look_from;
        let samples_per_pixel = config.samples_per_pixel;

        let width = config.image_width;
        let height = (width as f32 / config.aspect_ratio) as u32;

        let viewport_height = 2.0 * (config.fov.to_radians() / 2.0).tan() * config.focus_dist;
        let viewport_width = viewport_height * config.aspect_ratio;

        let w = (origin - config.look_at).normalize();
        let u = (config.vup.cross(w)).normalize(); // Vector que apunta hacia la derecha de la cámara.
        let v = u.cross(w).normalize(); //Vector que apunta hacia abajo de la cámara

        let pixel_delta_u = (viewport_width / width as f32) * u;
        let pixel_delta_v = (viewport_height / height as f32) * v;

        let viewport_upper_left = origin
            - config.focus_dist * w
            - (u * viewport_width * 0.5)
            - (v * viewport_height * 0.5);
        let pixel00_loc = viewport_upper_left + 0.5 * (pixel_delta_u + pixel_delta_v);

        let defocus_radius = config.focus_dist * (config.defocus_angle.to_radians() / 2.0).tan();
        let defocus_disk_u = u * defocus_radius;
        let defocus_disk_v = v * defocus_radius;

        Self {
            origin,
            pixel00_loc,
            pixel_delta_u,
            pixel_delta_v,
            samples_per_pixel,
            width,
            height,
            config,
            defocus_disk_u,
            defocus_disk_v,
        }
    }

    /// Genera un rayo dirigido al píxel (x, y).
    /// Si `sample > 0`, aplica un desfase aleatorio sub-píxel (Antialiasing).
    pub fn get_ray(&self, x: u32, y: u32, sample: u32, rng: &mut Rng) -> Ray {
        debug_assert!(
            x < self.width,
            "get_ray: x ({}) must be less than the camera width ({})",
            x,
            self.width
        );
        debug_assert!(
            y < self.height,
            "get_ray: y ({}) must be less than the camera height ({})",
            y,
            self.height
        );

        let offset = if sample == 0 {
            Vec3::new(0.0, 0.0, 0.0)
        } else {
            self.sample_square(rng)
        };

        let destination = self.pixel00_loc
            + (x as f32 + offset.x) * self.pixel_delta_u
            + (y as f32 + offset.y) * self.pixel_delta_v;

        let origin = if self.config.defocus_angle <= 0.0 {
            self.origin
        } else {
            let lens_sample = self.sample_disk_in_unit_circle(rng);
            self.origin
                + (lens_sample.x * self.defocus_disk_u)
                + (lens_sample.y * self.defocus_disk_v)
        };

        let direction = (destination - origin).normalize();

        Ray { origin, direction }
    }

    fn sample_square(&self, rng: &mut Rng) -> Vec3 {
        let rand_x = rng.f32() - 0.5;
        let rand_y = rng.f32() - 0.5;

        Vec3::new(rand_x, rand_y, 0.0)
    }

    fn sample_disk_in_unit_circle(&self, rng: &mut Rng) -> Vec3 {
        loop {
            // Generamos un punto en un cuadrado de [-1, 1] en X y Y
            let p = Vec3::new(
                rng.f32() * 2.0 - 1.0,
                rng.f32() * 2.0 - 1.0,
                0.0,
            );
            // Si el punto está dentro del círculo unitario (magnitud al cuadrado < 1), lo devolvemos
            if p.length_squared() < 1.0 {
                return p;
            }
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        let look_from = Point3::new(0.0, 0.0, 0.0);
        let look_at = Point3::new(0.0, 0.0, -1.0);
        let focus_dist = (look_at - look_from).length_squared();
        let config = CameraConfig {
            aspect_ratio: 16.0 / 9.0,
            image_width: 1920,
            fov: 90.0,
            look_from,
            look_at,
            vup: Point3::new(0.0, 1.0, 0.0),
            samples_per_pixel: 10,
            defocus_angle: 0.0,
            focus_dist,
        };
        Self::new(config)
    }
}
