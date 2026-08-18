use fastrand::Rng;
use rt_core::{Color, Interval, Ray};
use rt_scene::Scene;

use crate::stats::RayStats;
pub struct RayContext {
    pub rng: Rng,
    pub stats: RayStats,
}

pub trait RayTracer: Send + Sync + 'static {
    fn trace_ray(
        &self,
        ray: Ray,
        scene: &Scene,
        context: &mut RayContext,
    ) -> Color;
}

pub struct NormalTracer {}

impl RayTracer for NormalTracer {
    fn trace_ray(
        &self,
        ray: Ray,
        scene: &Scene,
        context: &mut RayContext,
    ) -> Color {
        let interval = Interval::new(0.0, f32::INFINITY);

        context.stats.rays += 1;
        if let Some(rec) = scene.world.hit(&ray, interval) {
            // Las mapeamos linealmente a [0.0, 1.0] para convertirlas en color.
            let r = 0.5 * (rec.normal.x + 1.0);
            let g = 0.5 * (rec.normal.y + 1.0);
            let b = 0.5 * (rec.normal.z + 1.0);

            return Color::new(r, g, b);
        }
        scene.background.emit(&ray)
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
    fn trace_ray(
        &self,
        ray: Ray,
        scene: &Scene,
        context: &mut RayContext,
    ) -> Color {
        let mut current_ray = ray;

        let mut attenuation = Color::ONE;

        let mut accumulated_light = Color::ZERO;

        for _ in 0..self.max_depth {
            let interval = Interval::new(0.001, f32::INFINITY);

            context.stats.rays += 1;
            if let Some(rec) = scene.world.hit(&current_ray, interval) {
                let material = scene.materials[rec.material as usize];
                let emitted = material.emitted(rec.normal[0], rec.normal[0], rec.p);
                accumulated_light += attenuation * emitted;

                if let Some((attenuation_material, scattered_ray)) =
                    material.scatter(&current_ray, &rec, &mut context.rng)
                {
                    attenuation *= attenuation_material;
                    current_ray = scattered_ray;
                } else {
                    return accumulated_light;
                }
            } else {
                return accumulated_light + attenuation * scene.background.emit(&current_ray);
            }
        }
        accumulated_light
    }
}
