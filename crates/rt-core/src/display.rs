use serde::{Deserialize, Serialize};

use crate::{Vec3, Vec4};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ToneMap {
    None,
    Reinhard,
    Aces,
}

impl ToneMap {
    pub fn apply(self, hdr: Vec3) -> Vec3 {
        match self {
            ToneMap::None => hdr,
            ToneMap::Reinhard => hdr / (Vec3::ONE + hdr),
            ToneMap::Aces => {
                let a = Vec3::splat(2.51);
                let b = Vec3::splat(0.03);
                let c = Vec3::splat(2.43);
                let d = Vec3::splat(0.59);
                let e = Vec3::splat(0.14);

                (hdr * (a * hdr + b)) / (hdr * (c * hdr + d) + e)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Transfer {
    Linear,
    Gamma(f32),
    Srgb,
}

impl Transfer {
    pub fn encode(self, display_referred: Vec3) -> Vec3 {
        match self {
            Transfer::Linear => display_referred,
            Transfer::Gamma(2.0) => display_referred.sqrt(),
            Transfer::Gamma(g) => {
                let inv = 1.0 / g;
                display_referred.powf(inv)
            }
            Transfer::Srgb => Vec3::new(
                srgb_encode(display_referred.x),
                srgb_encode(display_referred.y),
                srgb_encode(display_referred.z),
            ),
        }
    }
}

fn srgb_encode(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DisplayParams {
    pub exposure: f32,
    pub operator: ToneMap,
    pub transfer: Transfer,
}

impl Default for DisplayParams {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            operator: ToneMap::Aces,
            transfer: Transfer::Gamma(2.0),
        }
    }
}

pub fn resolve(buf: &[Vec4]) -> Vec<Vec3> {
    buf.iter()
        .map(|pixel| {
            let samples = pixel.w;
            if samples > 0.0 {
                Vec3::from_vec4(*pixel) / samples
            } else {
                Vec3::ZERO
            }
        })
        .collect()
}

pub fn to_srgb8(linear: &[Vec3], params: &DisplayParams) -> Vec<u8> {
    let mut out = Vec::with_capacity(linear.len() * 3);

    for &pixel in linear {
        let exposed = pixel * params.exposure;

        let mapped = params.operator.apply(exposed).clamp(Vec3::ZERO, Vec3::ONE);
        let encoded = params.transfer.encode(mapped);

        out.push(quantize(encoded.x));
        out.push(quantize(encoded.y));
        out.push(quantize(encoded.z));
    }

    out
}

fn quantize(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}
