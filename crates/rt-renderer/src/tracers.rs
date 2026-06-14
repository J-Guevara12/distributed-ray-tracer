use rt_core::{Color, Interval, Ray};
use rt_scene::Hittable;

pub trait RayTracer: Send + Sync + 'static {
    fn trace_ray(&self, ray: Ray, world: &dyn Hittable) -> Color;
}

pub struct NormalTracer {}

impl RayTracer for NormalTracer {
    fn trace_ray(&self, ray: Ray, world: &dyn Hittable) -> Color {
        let interval = Interval::new(0.0, f32::INFINITY);

        if let Some(rec) = world.hit(&ray, interval) {
            // Las mapeamos linealmente a [0.0, 1.0] para convertirlas en color.
            let r = 0.5 * (rec.normal.x + 1.0);
            let g = 0.5 * (rec.normal.y + 1.0);
            let b = 0.5 * (rec.normal.z + 1.0);

            return Color::new(r, g, b);
        }
        let unit_direction = ray.direction;

        let t = 0.5 * (unit_direction.y + 1.0);

        let r = (1.0 - t) * 1.0 + t * 0.5;
        let g = (1.0 - t) * 1.0 + t * 0.7;
        let b = (1.0 - t) * 1.0 + t * 1.0;

        Color::new(r, g, b)
    }
}

pub struct PathTracer {
    pub max_depth: u32,
}

impl PathTracer {
    pub fn new(max_depth: u32) -> Self {
        Self { max_depth }
    }
}

impl RayTracer for PathTracer {
    fn trace_ray(&self, ray: Ray, world: &dyn Hittable) -> Color {
        let mut current_ray = ray;

        let mut attenuation = Color::ONE;

        for _ in 0..self.max_depth {
            let interval = Interval::new(0.001, f32::INFINITY);

            if let Some(rec) = world.hit(&current_ray, interval) {
                if let Some((attenuation_material, scattered_ray)) =
                    rec.material.scatter(&current_ray, &rec)
                {
                    attenuation *= attenuation_material;
                    current_ray = scattered_ray;
                } else {
                    return Color::ZERO;
                }
            } else {
                let unit_direction = current_ray.direction;

                let t = 0.5 * (unit_direction.y + 1.0);
                let sky = Color::ONE * (1.0 - t) + Color::new(0.5, 0.7, 1.0) * t;

                return attenuation * sky;
            }
        }
        Color::ZERO
    }
}
