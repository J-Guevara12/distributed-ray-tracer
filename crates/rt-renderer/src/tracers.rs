use rt_core::{Color, Interval, Ray, background::Background};
use rt_scene::Hittable;

pub trait RayTracer: Send + Sync + 'static {
    fn trace_ray(&self, ray: Ray, world: &dyn Hittable, background: &Background) -> Color;
}

pub struct NormalTracer {}

impl RayTracer for NormalTracer {
    fn trace_ray(&self, ray: Ray, world: &dyn Hittable, background: &Background) -> Color {
        let interval = Interval::new(0.0, f32::INFINITY);

        if let Some(rec) = world.hit(&ray, interval) {
            // Las mapeamos linealmente a [0.0, 1.0] para convertirlas en color.
            let r = 0.5 * (rec.normal.x + 1.0);
            let g = 0.5 * (rec.normal.y + 1.0);
            let b = 0.5 * (rec.normal.z + 1.0);

            return Color::new(r, g, b);
        }
        background.emit(&ray)
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
    fn trace_ray(&self, ray: Ray, world: &dyn Hittable, background: &Background) -> Color {
        let mut current_ray = ray;

        let mut attenuation = Color::ONE;

        let mut accumulated_light = Color::ZERO;

        for _ in 0..self.max_depth {
            let interval = Interval::new(0.001, f32::INFINITY);

            if let Some(rec) = world.hit(&current_ray, interval) {
                let emitted = rec.material.emitted(rec.normal[0], rec.normal[0], rec.p);
                accumulated_light += attenuation * emitted;

                if let Some((attenuation_material, scattered_ray)) =
                    rec.material.scatter(&current_ray, &rec)
                {
                    attenuation *= attenuation_material;
                    current_ray = scattered_ray;
                } else {
                    return accumulated_light;
                }
            } else {
                return accumulated_light + attenuation * background.emit(&current_ray);
            }
        }
        accumulated_light
    }
}
