pub use glam::Vec3A;

pub mod dto;

pub type Vec3 = Vec3A;
pub type Color = Vec3A;
pub type Point3 = Vec3A;

#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Point3,
    pub direction: Vec3
}

impl Ray {
    pub fn new(origin: Point3, direction: Vec3) -> Self {
        Self {origin, direction: direction.normalize()}
    }

    pub fn at(&self, t: f32) -> Point3 {
        self.origin + self.direction * t
    }
}

#[derive(Debug, Clone, Copy)]
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
}

#[cfg(test)]
mod tests;
