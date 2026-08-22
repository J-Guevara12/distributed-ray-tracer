use rt_core::{Color, Interval, Ray, sampler::Sampler};
use rt_scene::Scene;

use crate::stats::RayStats;

const MIN_BOUNCES: u32 = 5;

/// Estimates the radiance arriving along a ray. The sampler is generic rather
/// than `dyn` on purpose: `next_1d` is called several times per bounce, and a
/// vtable there would defeat the reason `AnyIntegrator` is an enum.
pub trait Integrator: Send + Sync + 'static {
    fn radiance<S: Sampler>(
        &self,
        ray: Ray,
        scene: &Scene,
        sampler: &mut S,
        stats: &mut RayStats,
    ) -> Color;
}

pub enum AnyIntegrator {
    Normal(NormalTracer),
    Path(PathTracer),
}

impl AnyIntegrator {
    pub fn name(&self) -> &'static str {
        match self {
            AnyIntegrator::Normal(_) => "normal",
            AnyIntegrator::Path(_) => "path",
        }
    }
}

impl Integrator for AnyIntegrator {
    fn radiance<S: Sampler>(
        &self,
        ray: Ray,
        scene: &Scene,
        sampler: &mut S,
        stats: &mut RayStats,
    ) -> Color {
        match self {
            AnyIntegrator::Normal(integrator) => integrator.radiance(ray, scene, sampler, stats),
            AnyIntegrator::Path(integrator) => integrator.radiance(ray, scene, sampler, stats),
        }
    }
}

pub struct NormalTracer {}

impl Integrator for NormalTracer {
    fn radiance<S: Sampler>(
        &self,
        ray: Ray,
        scene: &Scene,
        _sampler: &mut S,
        stats: &mut RayStats,
    ) -> Color {
        let interval = Interval::new(0.0, f32::INFINITY);

        stats.rays += 1;
        if let Some(rec) = scene.world.hit_counted(&ray, interval, &mut stats.traversal) {
            // Mapped linearly to [0.0, 1.0] to be read as a colour.
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

impl Integrator for PathTracer {
    fn radiance<S: Sampler>(
        &self,
        ray: Ray,
        scene: &Scene,
        sampler: &mut S,
        stats: &mut RayStats,
    ) -> Color {
        let mut current_ray = ray;

        let mut attenuation = Color::ONE;

        let mut accumulated_light = Color::ZERO;

        for depth in 0..self.max_depth {
            let interval = Interval::new(0.001, f32::INFINITY);

            stats.rays += 1;
            if let Some(rec) =
                scene
                    .world
                    .hit_counted(&current_ray, interval, &mut stats.traversal)
            {
                let material = scene.materials[rec.material as usize];
                // (u, v) are texture coordinates; HitRecord won't have them until F2.7
                let emitted = material.emitted(0.0, 0.0, rec.p);
                accumulated_light += attenuation * emitted;

                if let Some((attenuation_material, scattered_ray)) =
                    material.scatter(&current_ray, &rec, sampler)
                {
                    attenuation *= attenuation_material;
                    current_ray = scattered_ray;
                } else {
                    return accumulated_light;
                }
            } else {
                return accumulated_light + attenuation * scene.background.emit(&current_ray);
            }
            if depth >= MIN_BOUNCES {
                let survive = attenuation.max_element().clamp(0.05, 1.0);
                if sampler.next_1d() >= survive {
                    return accumulated_light;
                }
                attenuation /= survive;
            }
        }
        accumulated_light
    }
}
