pub use glam::{Vec3A, Vec4, Vec2};

pub mod background;
pub mod camera;
pub mod dto;
pub mod job;
pub mod display;
pub mod sampler;

pub use job::*;

pub type Vec3 = Vec3A;
pub type Color = Vec3A;
pub type Point3 = Vec3A;

#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Point3,
    pub direction: Vec3,
    pub inv_dir: Vec3
}

impl Ray {
    pub fn new(origin: Point3, direction: Vec3) -> Self {
        let direction = direction.normalize();
        let inv_dir = 1.0/direction;
        Self { origin, direction, inv_dir }
    }

    pub fn at(&self, t: f32) -> Point3 {
        self.origin + self.direction * t
    }
}

pub fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}


#[derive(Debug, Clone, Copy, Default)]
pub struct Interval {
    pub min: f32,
    pub max: f32,
}

impl Interval {
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    pub fn contains(&self, x: f32) -> bool {
        self.min <= x && x <= self.max
    }

    pub fn surrounds(&self, x: f32) -> bool {
        self.min < x && x < self.max
    }

    pub fn expand(&self, delta: f32) -> Self {
        let min = self.min - delta;
        let max = self.max + delta;
        Self { min, max }
    }

    pub fn size(&self) -> f32 {
        self.max - self.min
    }
}

#[cfg(test)]
mod tests;
