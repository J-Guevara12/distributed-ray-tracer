use std::sync::Arc;

use rt_core::{Color, Interval, RayTracer};
use rt_scene::{Hittable, hittable_list::HittableList};

pub struct NormalTracer {
    pub world: Arc<HittableList>,
}

impl NormalTracer {
    pub fn new(world: Arc<HittableList>) -> Self {
        Self { world }
    }
}

impl RayTracer for NormalTracer {
    fn trace_ray(&self, ray: rt_core::Ray) -> Color {
        let interval = Interval::new(0.0, f32::INFINITY);

        if let Some(rec) = self.world.hit(&ray, interval) {
            // Las mapeamos linealmente a [0.0, 1.0] para convertirlas en color.
            let r = 0.5 * (rec.normal.x + 1.0);
            let g = 0.5 * (rec.normal.y + 1.0);
            let b = 0.5 * (rec.normal.z + 1.0);

            return Color::new( r, g, b );
        }
        let unit_direction = ray.direction;

        let t = 0.5 * (unit_direction.y + 1.0);

        let r = (1.0 - t) * 1.0 + t * 0.5;
        let g = (1.0 - t) * 1.0 + t * 0.7;
        let b = (1.0 - t) * 1.0 + t * 1.0;

        Color::new( r, g, b )
    }
}

