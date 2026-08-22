use fastrand::Rng;

use crate::{Vec2, splitmix64};

pub trait Sampler {
    fn start_sample(&mut self, pixel: (u32, u32), index: u32);
    fn next_1d(&mut self) -> f32;
    fn next_2d(&mut self) -> Vec2;
}

pub struct IndependentSampler {
    rng: Rng,
    width: u32,
}

impl IndependentSampler {
    pub fn new(rng: Rng, width: u32) -> Self {
        Self { rng, width }
    }

    /// Fixed seed for tests and one-off draws. `width` is 1, so `start_sample`
    /// stops separating pixels — callers of this constructor do not use it.
    pub fn with_seed(seed: u64) -> Self {
        Self::new(Rng::with_seed(seed), 1)
    }
}

impl Sampler for IndependentSampler {
    fn start_sample(&mut self, pixel: (u32, u32), index: u32) {
        let seed = ((pixel.1 as u64 * self.width as u64 + pixel.0 as u64) << 32) | index as u64;
        self.rng = Rng::with_seed(splitmix64(seed));
    }

    fn next_1d(&mut self) -> f32 {
        self.rng.f32()
    }

    fn next_2d(&mut self) -> Vec2 {
        Vec2::new(self.rng.f32(), self.rng.f32())
    }
}
